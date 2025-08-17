use std::collections::HashMap;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub channels: Vec<String>,
    pub ai_prompt: String,
    pub own_user_id: String,
    pub features: HashMap<String, bool>,
}

impl Config {
    pub fn load() -> Self {
        let content = std::fs::read_to_string("Config.toml")
            .expect("Could not read Config.toml");
        toml::from_str(&content).expect("Invalid Config.toml")
    }
}
