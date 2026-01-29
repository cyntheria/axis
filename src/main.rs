use anyhow::{Context, Result};
use clap::Parser;
use std::process;
use axis::args::ResamplerArgs;
use axis::audio;
use axis::resampler;

fn main() {
    env_logger::init();
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = ResamplerArgs::parse();
    
    let (samples, sample_rate) = audio::load_audio(&args.in_file)
        .with_context(|| format!("Failed to load audio from {}", args.in_file))?;
    
    if samples.is_empty() {
        audio::save_audio(&args.out_file, &[], sample_rate)
            .with_context(|| format!("Failed to save audio to {}", args.out_file))?;
        return Ok(());
    }
    
    let resampled = resampler::resample(&args, &samples, sample_rate)
        .context("Failed to resample audio")?;
    
    audio::save_audio(&args.out_file, &resampled, sample_rate)
        .with_context(|| format!("Failed to save audio to {}", args.out_file))?;
    
    Ok(())
}
