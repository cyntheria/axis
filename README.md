<p align="center">
  <img src="AXIS_White.png" width="400" alt="AXIS">
</p>

# AXIS

![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)
![License](https://img.shields.io/badge/license-LGPL--3.0-blue.svg)
[![Codacy Badge](https://app.codacy.com/project/badge/Grade/9bf4da58fe5c463fb1a92de324e50aca)](https://app.codacy.com/gh/cyntheria/axis/dashboard?utm_source=gh&utm_medium=referral&utm_content=&utm_campaign=Badge_grade)

**AXIS** is a high-performance, modern UTAU resampler written entirely in Rust. It utilizes the WORLD vocoder to provide (mostly) high-quality vocal synthesis, flexible pitch manipulation, and a robust plugin architecture.

## Features

- **Plugin System**: Extend AXIS with custom DSP or feature manipulation modules using shared libraries (`.so`).
- **Disk Caching**: Automatic feature extraction caching for near-instant rendering on repeat notes
- **Modern Config**: Plugin management via SQLite and configuration via KDL.

## Installation

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- SQLite dev libraries (for `rusqlite`)

### Building
```bash
git clone https://github.com/cyntheria/axis
cd axis
cargo build --release
```
The binary will be located at `target/release/axis`.

## Usage

### UTAU Integration
Copy the `axis` binary to the `Resamplers` folder of your UTAU installation (e.g., `~/.local/share/OpenUtau/Resamplers/` for OpenUtau).

AXIS follows the standard UTAU CLI protocol:
```bash
axis input.wav output.wav C4 100 0 0 1000 50 0 100 0 !120.0 [pitchbend_data]
```

### Plugin Management
AXIS includes a built-in CLI for managing plugins:

- **List plugins**: `axis plugin list`
- **Register a plugin**: `axis plugin add path/to/plugin.so`
- **Enable/Disable**: `axis plugin enable "Plugin Name"` / `axis plugin disable "Plugin Name"`
- **Remove**: `axis plugin remove "Plugin Name"`

## Developer API

AXIS provides a trait-based API for creating plugins. Plugins can hook into two stages of the pipeline:
1. **`process_features`**: Modify WORLD parameters (F0, MGC, BAP) before synthesis.
2. **`process_audio`**: Modify the final waveform after synthesis.

### Example Plugin
```rust
use axis::api::{AxisPlugin, PluginMetadata};

struct MyPlugin;

impl AxisPlugin for MyPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "My Plugin".into(),
            version: "0.1.0".into(),
            author: "You".into(),
            description: "An example plugin".into(),
        }
    }

    fn process_audio(&mut self, samples: &mut [f64], _sample_rate: u32) -> anyhow::Result<()> {
        // Your DSP here
        Ok(())
    }
}
```

## License

This project is licensed under the LGPL-3.0 License - see the [LICENSE](LICENSE) file for details.
