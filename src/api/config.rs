use knuffel::Decode;
use serde::{Deserialize, Serialize};

#[derive(Decode, Debug, Clone, Serialize, Deserialize)]
pub struct AxisConfig {
    #[knuffel(child)]
    pub general: Option<GeneralConfig>,
    #[knuffel(children(name = "plugin"))]
    pub plugins: Vec<PluginConfig>,
}

#[derive(Decode, Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[knuffel(property)]
    pub threads: Option<i32>,
    #[knuffel(property)]
    pub analysis_enabled: Option<bool>,
    #[knuffel(property)]
    pub log: Option<bool>,
    #[knuffel(property)]
    pub stydl: Option<bool>,
}

#[derive(Decode, Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    #[knuffel(argument)]
    pub name: String,
    #[knuffel(property)]
    pub enabled: bool,
    #[knuffel(children)]
    pub settings: Vec<Setting>,
}

#[derive(Decode, Debug, Clone, Serialize, Deserialize)]
pub struct Setting {
    #[knuffel(argument)]
    pub key: String,
    #[knuffel(argument)]
    pub value: String,
}

impl AxisConfig {
    pub fn load<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config = knuffel::parse("config.kdl", &content)?;
        Ok(config)
    }
}

impl Default for AxisConfig {
    fn default() -> Self {
        Self {
            general: Some(GeneralConfig {
                threads: Some(0),
                analysis_enabled: Some(true),
                log: Some(true),
                stydl: Some(true),
            }),
            plugins: Vec::new(),
        }
    }
}
