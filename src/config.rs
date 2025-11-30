use std::{env, path::PathBuf};

use openrgb2::Color as OrgbColor;
use serde::{Deserialize, Serialize};

pub fn get_config_dir() -> PathBuf {
    let home = env::home_dir().unwrap();
    home.join(PathBuf::from_iter([".config", "openrgb-fade"]))
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Deserialize, Serialize)]
pub struct SDKServerInfo {
    pub address: Option<String>,
    pub port: Option<u16>,
}

impl From<Color> for OrgbColor {
    fn from(value: Color) -> Self {
        Self::new(value.r, value.g, value.b)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    color: Color,
    fps: Option<usize>,
    fadeout_time_ms: Option<usize>,
    server: Option<SDKServerInfo>,
}

pub const DEFAULT_CONFIG: &str = include_str!("default_config.jsonc");

impl Config {
    pub fn color(&self) -> OrgbColor {
        OrgbColor::from(self.color)
    }

    pub fn fps(&self) -> usize {
        self.fps.unwrap_or(60)
    }

    // TODO: implement
    pub fn fadeout_time_ms(&self) -> usize {
        self.fadeout_time_ms.unwrap_or(1000)
    }

    pub fn load_from_first() -> Option<Self> {
        let path = get_config_dir().join(PathBuf::from("config.jsonc"));

        if std::fs::exists(&path).unwrap_or(false)
            && let Ok(contents) = std::fs::read_to_string(&path)
        {
            match serde_jsonc::from_str::<Config>(&contents) {
                Err(_) => return None,
                Ok(config) => return Some(config),
            }
        }

        println!(
            "No config file found. Writing default to {}",
            path.to_string_lossy()
        );

        std::fs::create_dir_all(get_config_dir()).unwrap();
        std::fs::write(path, DEFAULT_CONFIG).unwrap();
        serde_jsonc::from_str(DEFAULT_CONFIG).ok()
    }
}
