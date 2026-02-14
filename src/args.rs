use crate::util::{pitch_parser, tempo_parser};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "WORLD-based UTAU resampler on Rust.")]
#[command(disable_help_flag = true)]
#[command(disable_version_flag = true)]
#[command(allow_negative_numbers = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(index = 1)]
    pub in_file: Option<String>,
    #[arg(index = 2)]
    pub out_file: Option<String>,
    #[arg(index = 3, value_parser = pitch_parser)]
    pub pitch: Option<i32>,
    #[arg(index = 4)]
    pub velocity: Option<f64>,
    #[arg(index = 5)]
    pub flags: Option<String>,
    #[arg(index = 6)]
    pub offset: Option<f64>,
    #[arg(index = 7)]
    pub length: Option<f64>,
    #[arg(index = 8)]
    pub consonant: Option<f64>,
    #[arg(index = 9)]
    pub cutoff: Option<f64>,
    #[arg(index = 10)]
    pub volume: Option<f64>,
    #[arg(index = 11)]
    pub modulation: Option<f64>,
    #[arg(index = 12, value_parser = tempo_parser)]
    pub tempo: Option<f64>,
    #[arg(index = 13)]
    pub pitchbend: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
}

#[derive(Subcommand)]
pub enum PluginAction {
    List,
    Add { path: String },
    Remove { name: String },
    Enable { name: String },
    Disable { name: String },
}

pub struct ResamplerArgs {
    pub in_file: String,
    pub out_file: String,
    pub pitch: i32,
    pub velocity: f64,
    pub flags: String,
    pub offset: f64,
    pub length: f64,
    pub consonant: f64,
    pub cutoff: f64,
    pub volume: f64,
    pub modulation: f64,
    pub tempo: f64,
    pub pitchbend: Option<String>,
}

impl Cli {
    pub fn to_resampler_args(self) -> Option<ResamplerArgs> {
        Some(ResamplerArgs {
            in_file: self.in_file?,
            out_file: self.out_file?,
            pitch: self.pitch?,
            velocity: self.velocity?,
            flags: self.flags?,
            offset: self.offset?,
            length: self.length?,
            consonant: self.consonant?,
            cutoff: self.cutoff?,
            volume: self.volume?,
            modulation: self.modulation?,
            tempo: self.tempo?,
            pitchbend: self.pitchbend,
        })
    }
}
