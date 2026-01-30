use anyhow::{Result};
use crate::args::ResamplerArgs;
use crate::util::{decode_pitchbend, midi_to_hz, arange, linspace, lerp};
use crate::flags::Flags;
use rsworld::{harvest, stonemask, cheaptrick, d4c, synthesis, code_spectral_envelope, decode_spectral_envelope, code_aperiodicity, decode_aperiodicity};
use rsworld_sys::{HarvestOption, CheapTrickOption, D4COption};
use std::str::FromStr;
use log::{info, debug};
use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{Read, Write};

const FRAME_PERIOD: f64 = 5.0;

#[derive(Serialize, Deserialize)]
struct WorldFeatures {
    f0: Vec<f64>,
    mgc: Vec<Vec<f64>>,
    bap: Vec<Vec<f64>>,
    source_base_hz: f64,
    fft_size: i32,
}

fn apply_volume(samples: &mut [f64], volume: f64) {
    let scale = volume / 100.0;
    for sample in samples.iter_mut() {
        *sample *= scale;
    }
}

fn get_cache_path(source: &str) -> PathBuf {
    let path = Path::new(source);
    let mut cache = path.to_path_buf();
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".axis_cache");
    cache.set_file_name(name);
    cache
}

pub fn resample(
    args: &ResamplerArgs, 
    input_samples: &[f64], 
    sample_rate: u32,
    plugins: &mut [&mut dyn crate::api::AxisPlugin]
) -> Result<Vec<f64>> {
    if input_samples.is_empty() {
        return Ok(vec![]);
    }

    info!("Starting resampling: pitch={}Hz (MIDI {}), tempo={}", midi_to_hz(args.pitch as f64), args.pitch, args.tempo);
    
    let velocity = (1.0 - args.velocity / 100.0).exp2();
    let modulation = args.modulation / 100.0;
    let flags = Flags::from_str(&args.flags).unwrap_or(Flags { gender: 0.0, breathiness: 50.0 });
    
    debug!("Flags applied: gender={}, breathiness={}", flags.gender, flags.breathiness);

    let fs = sample_rate as i32;
    let cache_path = get_cache_path(&args.in_file);
    
    let features = if cache_path.exists() {
        info!("Loading cached features from {}", cache_path.display());
        let mut f = File::open(&cache_path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        bincode::deserialize::<WorldFeatures>(&buf)?
    } else {
        info!("Running WORLD analysis...");
        let x = input_samples.to_vec();
        let harvest_option = HarvestOption::new();
        let (temporal_positions, f0_raw) = harvest(&x, fs, &harvest_option);
        let f0 = stonemask(&x, fs, &temporal_positions, &f0_raw);
        
        let mut ct_option = CheapTrickOption::new(fs);
        let spec = cheaptrick(&x, fs, &temporal_positions, &f0, &mut ct_option);
        
        let d4c_option = D4COption::new();
        let ap = d4c(&x, fs, &temporal_positions, &f0, &d4c_option);

        let fft_size = ct_option.fft_size;
        let f0_len = f0.len() as i32;
        
        let mut voiced_f0: Vec<f64> = f0.iter().cloned().filter(|&f| f > 0.0).collect();
        voiced_f0.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let source_base_hz = if voiced_f0.is_empty() { 261.63 } else { voiced_f0[voiced_f0.len() / 2] };
        
        info!("Analysis complete. Frames: {}, FFT size: {}, Median F0: {:.2}Hz", f0_len, fft_size, source_base_hz);

        let mgc = code_spectral_envelope(&spec, f0_len, fs, fft_size, 64);
        let bap = code_aperiodicity(&ap, f0_len, fs);

        let feats = WorldFeatures { f0, mgc, bap, source_base_hz, fft_size };
        let bin = bincode::serialize(&feats)?;
        let mut f = File::create(&cache_path)?;
        f.write_all(&bin)?;
        feats
    };

    let f0_len = features.f0.len();
    let f0_off: Vec<f64> = features.f0.iter().map(|&f| if f == 0.0 { 0.0 } else { 12.0 * (f.log2() - features.source_base_hz.log2()) }).collect();

    let fps = 1000.0 / FRAME_PERIOD;
    let feature_length_sec = f0_len as f64 / fps;

    let start = args.offset / 1000.0;
    let end = if args.cutoff < 0.0 { start - args.cutoff / 1000.0 } else { feature_length_sec - args.cutoff / 1000.0 };
    let consonant_src = start + args.consonant / 1000.0;

    let t_consonant = linspace(start, consonant_src, (velocity * args.consonant / FRAME_PERIOD) as usize, false);
    let length_req = args.length / 1000.0;
    let stretch_length = end - consonant_src;
    let t_stretch = if stretch_length > length_req {
        let con_idx = (consonant_src * fps) as usize;
        let len_idx = (length_req * fps) as usize;
        (con_idx..(con_idx + len_idx).min(f0_len - 1)).map(|i| i as f64 / fps).collect()
    } else {
        linspace(consonant_src, end, (length_req * fps) as usize, true)
    };

    let t_render: Vec<f64> = t_consonant.into_iter().chain(t_stretch.into_iter()).map(|x| (x * fps).clamp(0.0, (f0_len - 1) as f64)).collect();
    let render_length = t_render.len();
    let t_sec: Vec<f64> = arange(render_length as i32).iter().map(|x| x / fps).collect();

    let mut f0_off_render = Vec::with_capacity(render_length);
    let mut mgc_render: Vec<Vec<f64>> = Vec::with_capacity(render_length);
    let mut bap_render: Vec<Vec<f64>> = Vec::with_capacity(render_length);
    let vuv_render: Vec<bool> = t_render.iter().map(|&t| features.f0[t as usize] != 0.0).collect();

    for &t in &t_render {
        let idx0 = t.floor() as usize;
        let idx1 = (idx0 + 1).min(f0_len - 1);
        let weight = t - idx0 as f64;
        f0_off_render.push(lerp(f0_off[idx0], f0_off[idx1], weight));
        mgc_render.push((0..features.mgc[0].len()).map(|i| lerp(features.mgc[idx0][i], features.mgc[idx1][i], weight)).collect());
        bap_render.push((0..features.bap[0].len()).map(|i| lerp(features.bap[idx0][i], features.bap[idx1][i], weight)).collect());
    }

    if flags.gender != 0.0 {
        let shift = (flags.gender / 120.0).exp2();
        for frame in mgc_render.iter_mut() {
            let orig = frame.clone();
            let len = frame.len();
            for i in 0..len {
                let s_idx = i as f64 * shift;
                let i0 = s_idx.floor() as usize;
                let i1 = (i0 + 1).min(len - 1);
                frame[i] = if i0 < len { lerp(orig[i0], orig[i1], s_idx - i0 as f64) } else { 0.0 };
            }
        }
    }

    let pb = args.pitchbend.as_deref().map(decode_pitchbend).unwrap_or_default();
    let pps = 8.0 * args.tempo / 5.0;
    
    let f0_render: Vec<f64> = (0..render_length).map(|i| {
        if !vuv_render[i] { return 0.0; }
        let t_p = t_sec[i] * pps;
        let pb_v = if pb.is_empty() { 0.0 } else {
            let idx = t_p.floor() as usize;
            if idx + 1 < pb.len() { lerp(pb[idx], pb[idx + 1], t_p - idx as f64) } else { *pb.last().unwrap() }
        };
        midi_to_hz(args.pitch as f64 + pb_v + f0_off_render[i] * modulation)
    }).collect();

    if flags.breathiness != 50.0 {
        let mix = (flags.breathiness / 100.0).clamp(0.0, 1.0);
        for frame in bap_render.iter_mut() {
            for val in frame.iter_mut() { *val = lerp(*val, 1.0, mix); }
        }
    }

    let mut f0_p = f0_render;
    let mut spec_p = mgc_render;
    let mut ap_p = bap_render;

    for plugin in plugins.iter_mut() {
        plugin.process_features(&mut f0_p, &mut spec_p, &mut ap_p, sample_rate)?;
    }

    let spec_r = decode_spectral_envelope(&spec_p, render_length as i32, fs, features.fft_size);
    let ap_r = decode_aperiodicity(&ap_p, render_length as i32, fs);
    let mut syn = synthesis(&f0_p, &spec_r, &ap_r, FRAME_PERIOD, fs);

    for plugin in plugins.iter_mut() {
        plugin.process_audio(&mut syn, sample_rate)?;
    }

    apply_volume(&mut syn, args.volume);
    
    let _ = crate::filter::apply_vocal_enhancement(&mut syn, sample_rate);
    
    info!("Resampling complete. Output: {} samples", syn.len());
    Ok(syn)
}
