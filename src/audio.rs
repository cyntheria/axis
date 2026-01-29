use anyhow::{Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, ReadOnlySource};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::get_codecs;
use symphonia::default::get_probe;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use log::{info, debug};

pub fn load_audio<P: AsRef<Path>>(path: P) -> Result<(Vec<f64>, u32)> {
    let path = path.as_ref();
    info!("Loading audio from {}", path.display());
    
    let file = File::open(path)
        .with_context(|| format!("Failed to open audio file: {}", path.display()))?;
    
    let mss = MediaSourceStream::new(Box::new(ReadOnlySource::new(BufReader::new(file))), Default::default());
    
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();
    
    let probed = get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .with_context(|| "Failed to probe audio format")?;
    
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .with_context(|| "No supported audio tracks found")?;
    
    let track_id = track.id;
    let codec_params = &track.codec_params;
    let sample_rate = codec_params.sample_rate.unwrap_or(44100);
    info!("Audio sample rate: {}Hz", sample_rate);
    
    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = get_codecs()
        .make(&codec_params, &dec_opts)
        .with_context(|| "Failed to create decoder")?;
    
    let mut samples = Vec::new();
    
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::ResetRequired) => {
                debug!("Decoder reset required");
                continue;
            }
            Err(_) => break,
        };
        
        if packet.track_id() != track_id {
            continue;
        }
        
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let duration = decoded.capacity() as u64;
                
                if duration == 0 {
                    continue;
                }
                
                let channels = spec.channels.count();
                
                let mut sample_buf = SampleBuffer::<f64>::new(duration, spec);
                sample_buf.copy_interleaved_ref(decoded);
                
                if channels > 1 {
                    let interleaved = sample_buf.samples();
                    let mono_samples: Vec<f64> = interleaved
                        .chunks(channels)
                        .map(|chunk| chunk.iter().sum::<f64>() / channels as f64)
                        .collect();
                    samples.extend_from_slice(&mono_samples);
                } else {
                    samples.extend_from_slice(sample_buf.samples());
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => {
                debug!("Decode error encountered, skipping packet");
                continue;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => {
                debug!("Decoder reset required during decode");
                continue;
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Decode error: {}", e));
            }
        }
    }
    
    info!("Loaded {} samples", samples.len());
    Ok((samples, sample_rate))
}

pub fn save_audio<P: AsRef<Path>>(
    path: P,
    samples: &[f64],
    sample_rate: u32,
) -> Result<()> {
    let path = path.as_ref();
    info!("Saving audio to {}", path.display());
    
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("wav");
    
    match ext.to_lowercase().as_str() {
        "wav" => write_wav(path, samples, sample_rate),
        _ => write_wav(path, samples, sample_rate),
    }
}

fn write_wav<P: AsRef<Path>>(path: P, samples: &[f64], sample_rate: u32) -> Result<()> {
    use std::io::Write;
    
    let mut file = File::create(path)?;
    
    let num_channels = 1u16;
    let bits_per_sample = 16u16;
    let byte_rate = sample_rate as u32 * num_channels as u32 * (bits_per_sample / 8) as u32;
    let block_align = num_channels * (bits_per_sample / 8);
    let data_size = samples.len() * 2;
    
    debug!("Writing WAV: channels={}, bits={}, rate={}, size={}", num_channels, bits_per_sample, sample_rate, data_size);

    file.write_all(b"RIFF")?;
    file.write_all(&((36 + data_size) as u32).to_le_bytes())?;
    file.write_all(b"WAVE")?;
    
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&num_channels.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&bits_per_sample.to_le_bytes())?;
    
    file.write_all(b"data")?;
    file.write_all(&(data_size as u32).to_le_bytes())?;
    
    if samples.is_empty() {
        let silent: i16 = 0;
        file.write_all(&silent.to_le_bytes())?;
    } else {
        for &sample in samples {
            let clamped = sample.max(-1.0).min(1.0);
            let int_sample = (clamped * 32767.0) as i16;
            file.write_all(&int_sample.to_le_bytes())?;
        }
    }
    
    Ok(())
}
