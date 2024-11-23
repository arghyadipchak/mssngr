use std::{
  env, fs, io,
  net::{IpAddr, Ipv4Addr},
  time::Duration,
};

use serde::{Deserialize, Deserializer};
use url::Url;

const fn default_host() -> IpAddr {
  IpAddr::V4(Ipv4Addr::LOCALHOST)
}

const fn default_port() -> u16 {
  8080
}

const fn default_max_queue() -> usize {
  1000
}

const fn default_workers() -> usize {
  4
}

fn default_persistence() -> Duration {
  Duration::from_secs(300)
}

#[derive(Deserialize)]
pub struct Config {
  pub id: String,

  #[serde(default = "default_host")]
  pub host: IpAddr,

  #[serde(default = "default_port")]
  pub port: u16,

  #[serde(default)]
  pub topics: Vec<String>,

  #[serde(default = "default_max_queue")]
  pub max_queue: usize,

  #[serde(default = "default_workers")]
  pub workers: usize,

  #[serde(default)]
  pub forward: Vec<Node>,

  #[serde(
    default = "default_persistence",
    deserialize_with = "deserialize_duration"
  )]
  pub persistence: Duration,
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
  D: Deserializer<'de>,
{
  Ok(Duration::from_secs(u64::deserialize(deserializer)?) * 60)
}

#[derive(Deserialize)]
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
