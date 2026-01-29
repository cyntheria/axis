use crate::util::{pitch_parser, tempo_parser};
use clap::Parser;

#[derive(Parser)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "WORLD-based UTAU resampler on Rust.")]
#[command(disable_help_flag = true)]
#[command(disable_version_flag = true)]
#[command(allow_negative_numbers = true)]
pub struct ResamplerArgs {
    #[arg(index = 1)]
    pub in_file: String,
    #[arg(index = 2)]
    pub out_file: String,
    #[arg(index = 3, value_parser = pitch_parser)]
    pub pitch: i32,
    #[arg(index = 4)]
    pub velocity: f64,
    #[arg(index = 5)]
    pub flags: String,
    #[arg(index = 6)]
    pub offset: f64,
    #[arg(index = 7)]
    pub length: f64,
    #[arg(index = 8)]
    pub consonant: f64,
    #[arg(index = 9)]
    pub cutoff: f64,
    #[arg(index = 10)]
    pub volume: f64,
    #[arg(index = 11)]
    pub modulation: f64,
    #[arg(index = 12, value_parser = tempo_parser)]
    pub tempo: f64,
    #[arg(index = 13, required = false)]
    pub pitchbend: Option<String>,
}
