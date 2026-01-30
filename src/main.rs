use anyhow::{Context, Result};
use clap::Parser;
use std::process;
use axis::args::{Cli, Commands, PluginAction};
use axis::api::PluginDatabase;
use axis::audio;
use axis::resampler;
use directories::ProjectDirs;

fn main() {
    env_logger::init();
    if let Err(e) = run() {
        log::error!("Error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let proj_dirs = ProjectDirs::from("com", "cyntheria", "axis")
        .context("Could not determine project directories")?;
    let db_path = proj_dirs.data_dir().join("plugin.db");
    std::fs::create_dir_all(proj_dirs.data_dir())?;
    
    let db = PluginDatabase::open(&db_path)?;

    if let Some(command) = cli.command {
        match command {
            Commands::Plugin { action } => match action {
                PluginAction::List => {
                    let plugins = db.list_plugins()?;
                    if plugins.is_empty() {
                        println!("No plugins registered.");
                    } else {
                        for (meta, path, enabled) in plugins {
                            println!("{} v{} ({}): {} [Enabled: {}]", meta.name, meta.version, path, meta.description, enabled);
                        }
                    }
                }
                PluginAction::Add { path } => {
                    let full_path = std::fs::canonicalize(&path)
                        .with_context(|| format!("Failed to resolve plugin path: {}", path))?;
                    
                    unsafe {
                        let mut loader = axis::api::PluginLoader::load(&full_path)
                            .with_context(|| format!("Failed to load plugin: {:?}", full_path))?;
                        let meta = loader.plugin().metadata();
                        db.register_plugin(&meta, full_path.to_str().unwrap())?;
                        println!("Registered plugin: {} v{} from {:?}", meta.name, meta.version, full_path);
                    }
                }
                PluginAction::Remove { name } => {
                    db.remove_plugin(&name)?;
                    println!("Removed plugin: {}", name);
                }
                PluginAction::Enable { name } => {
                    db.set_plugin_enabled(&name, true)?;
                    println!("Enabled plugin: {}", name);
                }
                PluginAction::Disable { name } => {
                    db.set_plugin_enabled(&name, false)?;
                    println!("Disabled plugin: {}", name);
                }
            },
        }
        return Ok(());
    }

    let args = cli.to_resampler_args()
        .context("No subcommand provided and resampling arguments are incomplete")?;

    let mut loaders = Vec::new();
    let plugins_info = db.list_plugins()?;
    for (meta, path, enabled) in plugins_info {
        if enabled {
            log::info!("Loading plugin: {} v{} from {}", meta.name, meta.version, path);
            unsafe {
                match axis::api::PluginLoader::load(&path) {
                    Ok(loader) => loaders.push(loader),
                    Err(e) => log::error!("Failed to load plugin {}: {}", meta.name, e),
                }
            }
        }
    }

    let mut plugin_refs: Vec<&mut dyn axis::api::AxisPlugin> = loaders
        .iter_mut()
        .map(|l| l.plugin())
        .collect();

    let (samples, sample_rate) = audio::load_audio(&args.in_file)
        .with_context(|| format!("Failed to load audio from {}", args.in_file))?;
    
    if samples.is_empty() {
        audio::save_audio(&args.out_file, &[], sample_rate)
            .with_context(|| format!("Failed to save audio to {}", args.out_file))?;
        return Ok(());
    }
    
    let resampled = resampler::resample(&args, &samples, sample_rate, &mut plugin_refs)
        .context("Failed to resample audio")?;
    
    audio::save_audio(&args.out_file, &resampled, sample_rate)
        .with_context(|| format!("Failed to save audio to {}", args.out_file))?;
    
    Ok(())
}
