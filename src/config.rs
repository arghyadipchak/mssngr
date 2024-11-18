use std::{
  env, fs, io,
  net::{IpAddr, Ipv4Addr},
};

use serde::Deserialize;
use url::Url;

const fn default_host() -> IpAddr {
  IpAddr::V4(Ipv4Addr::LOCALHOST)
}

const fn default_port() -> u16 {
  8080
}

#[derive(Debug, Deserialize)]
pub struct Config {
  pub id: String,

  #[serde(default = "default_host")]
  pub host: IpAddr,

  #[serde(default = "default_port")]
  pub port: u16,

  #[serde(default)]
  pub topics: Vec<String>,

  #[serde(default)]
  pub forward: Vec<Node>,
}

#[derive(Debug, Deserialize)]
pub struct Node {
  pub id: String,
  pub addr: Url,
  pub topics: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("config error :: {0}")]
pub enum ConfigError {
  IO(#[from] io::Error),
  Toml(#[from] toml::de::Error),
}

impl Config {
  pub fn read() -> Result<Self, ConfigError> {
    let config_path =
      env::var("MSSNGR_CONFIG").unwrap_or_else(|_| "config.toml".to_string());

    Ok(toml::from_str(&fs::read_to_string(config_path)?)?)
  }
}
