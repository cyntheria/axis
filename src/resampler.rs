use anyhow::Result;
use crate::args::ResamplerArgs;
use crate::util::{decode_pitchbend, midi_to_hz, arange, linspace, lerp};
use crate::flags::Flags;
use crate::vocoder::stydl::StydlVocoder;
use std::str::FromStr;
use log::{info, debug};
use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{Read, Write};

const FRAME_PERIOD: f64 = 5.0;

#[derive(Serialize, Deserialize)]
struct AxisFeatures {
    f0: Vec<f64>,
    spec: Vec<Vec<f64>>,
    ap: Vec<Vec<f64>>,
    source_base_hz: f64,
    fft_size: usize,
}

fn apply_volume(samples: &mut [f64], volume: f64) {
    let scale = volume / 100.0;
    for sample in samples.iter_mut() {
        *sample *= scale;
    }
}

fn get_analysis_path(source: &str) -> PathBuf {
    let path = Path::new(source);
    let mut analysis = path.to_path_buf();
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".axxf");
    analysis.set_file_name(name);
    analysis
}

pub fn resample(
    args: &ResamplerArgs, 
    input_samples: &[f64], 
    sample_rate: u32,
    plugins: &mut [&mut dyn crate::api::AxisPlugin],
    _config: &crate::api::AxisConfig,
) -> Result<Vec<f64>> {
    if input_samples.is_empty() {
        return Ok(vec![]);
    }

    info!("Starting resampling [STYDL]: pitch={}Hz (MIDI {}), tempo={}", midi_to_hz(args.pitch as f64), args.pitch, args.tempo);
    
    let velocity = (1.0 - args.velocity as f64 / 100.0).exp2();
    let modulation = args.modulation / 100.0;
    let flags = Flags::from_str(&args.flags).unwrap_or(Flags { gender: 0.0, breathiness: 50.0 });
    
    debug!("Flags applied: gender={}, breathiness={}", flags.gender, flags.breathiness);

    let analysis_path = get_analysis_path(&args.in_file);
    
    let features = if analysis_path.exists() {
        info!("Loading analysis data from {}", analysis_path.display());
        let mut f = File::open(&analysis_path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        bincode::deserialize::<AxisFeatures>(&buf)?
    } else {
        info!("Running STYDL analysis...");
        let vocoder = StydlVocoder::new(sample_rate, 4096);
        let fft_size = vocoder.fft_size;
        
        let hop_size = (sample_rate as f64 * FRAME_PERIOD / 1000.0) as usize;
        let num_frames = input_samples.len() / hop_size;
        
        let mut f0 = vec![0.0; num_frames];
        let mut spec = Vec::with_capacity(num_frames);
        let mut ap = Vec::with_capacity(num_frames);

        // 1. F0 Estimation
        f0 = vocoder.f0_estimator.estimate(input_samples);
        f0.truncate(num_frames); // Align

        // 2. Spectral & Aperiodicity Estimation
        for i in 0..f0.len() {
            let start = i * hop_size;
            let end = (start + fft_size).min(input_samples.len());
            let chunk = &input_samples[start..end];
            
            spec.push(vocoder.spectral_resolver.resolve(chunk, f0[i], fft_size));
            ap.push(vocoder.aperiodicity_estimator.estimate(chunk, f0[i], fft_size));
        }

        let mut voiced_f0: Vec<f64> = f0.iter().cloned().filter(|&f| f > 40.0).collect();
        voiced_f0.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let source_base_hz = if voiced_f0.is_empty() { 261.63 } else { voiced_f0[voiced_f0.len() / 2] };
        
        info!("Analysis complete. Frames: {}, FFT size: {}, Median F0: {:.2}Hz", f0.len(), fft_size, source_base_hz);

        let feats = AxisFeatures { f0, spec, ap, source_base_hz, fft_size };
        let bin = bincode::serialize(&feats)?;
        let mut f = File::create(&analysis_path)?;
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

    let t_render: Vec<f64> = t_consonant.into_iter().chain(t_stretch.into_iter()).map(|x: f64| (x * fps).clamp(0.0, (f0_len - 1) as f64)).collect();
    let render_length = t_render.len();
    let t_sec: Vec<f64> = arange(render_length as i32).iter().map(|x| x / fps).collect();

    let mut f0_off_render = Vec::with_capacity(render_length);
    let mut spec_render: Vec<Vec<f64>> = Vec::with_capacity(render_length);
    let mut ap_render: Vec<Vec<f64>> = Vec::with_capacity(render_length);
    let vuv_render: Vec<bool> = t_render.iter().map(|&t: &f64| features.f0[t as usize] != 0.0).collect();

    for &t in &t_render {
        let idx0 = t.floor() as usize;
        let idx1 = (idx0 + 1).min(f0_len - 1);
        let weight = t - idx0 as f64;
        f0_off_render.push(lerp(f0_off[idx0], f0_off[idx1], weight));
        spec_render.push((0..features.spec[0].len()).map(|i| lerp(features.spec[idx0][i], features.spec[idx1][i], weight)).collect());
        ap_render.push((0..features.ap[0].len()).map(|i| lerp(features.ap[idx0][i], features.ap[idx1][i], weight)).collect());
    }

    if flags.gender != 0.0 {
        let shift = (flags.gender / 120.0).exp2();
        for frame in spec_render.iter_mut() {
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
        for frame in ap_render.iter_mut() {
            for val in frame.iter_mut() { *val = lerp(*val, 1.0, mix); }
        }
    }

    let mut f0_p = f0_render;
    let mut spec_p = spec_render;
    let mut ap_p = ap_render;

    for plugin in plugins.iter_mut() {
        plugin.process_features(&mut f0_p, &mut spec_p, &mut ap_p, sample_rate)?;
    }

    // Smooth spectrum (internal tool)
    for frame in spec_p.iter_mut() {
        crate::util::smooth_spectrum(frame, 3);
    }

    for i in 0..render_length {
        if f0_p[i] == 0.0 {
            for val in ap_p[i].iter_mut() { *val = 1.0; }
        }
    }

    info!("Using STYDL vocoder for synthesis...");
    let mut vocoder = StydlVocoder::new(sample_rate, features.fft_size);
    let mut syn = vocoder.process(&f0_p, &spec_p, &ap_p, input_samples, &t_render);

    for plugin in plugins.iter_mut() {
        plugin.process_audio(&mut syn, sample_rate)?;
    }

    apply_volume(&mut syn, args.volume);
    
    let _ = crate::filter::apply_vocal_enhancement(&mut syn, sample_rate);
    
    info!("Resampling complete. Output: {} samples", syn.len());
    Ok(syn)
}
