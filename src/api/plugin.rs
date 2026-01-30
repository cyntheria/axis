use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
}

pub trait AxisPlugin: Send + Sync {
    fn metadata(&self) -> PluginMetadata;
    fn on_load(&mut self) -> anyhow::Result<()> { Ok(()) }
    fn on_unload(&mut self) -> anyhow::Result<()> { Ok(()) }
    
    fn process_audio(&mut self, _samples: &mut [f64], _sample_rate: u32) -> anyhow::Result<()> {
        Ok(())
    }

    fn process_features(
        &mut self,
        _f0: &mut [f64],
        _spectral: &mut [Vec<f64>],
        _aperiodicity: &mut [Vec<f64>],
        _sample_rate: u32,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

pub struct PluginLoader {
    _lib: libloading::Library,
    plugin: Box<dyn AxisPlugin>,
}

impl PluginLoader {
    pub unsafe fn load<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<Self> {
        let lib = libloading::Library::new(path.as_ref())?;
        
        let constructor: libloading::Symbol<fn() -> Box<dyn AxisPlugin>> = 
            lib.get(b"_axis_plugin_create")?;
            
        let plugin = constructor();
        
        Ok(Self {
            _lib: lib,
            plugin,
        })
    }

    pub fn plugin(&mut self) -> &mut dyn AxisPlugin {
        self.plugin.as_mut()
    }
}
