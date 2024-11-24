use std::{
  env, fs, io,
  net::{IpAddr, Ipv4Addr},
  time::Duration,
};

use regex::Regex;
use serde::{de, Deserialize, Deserializer};
use url::Url;

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

  #[serde(
    default = "default_persistence",
    deserialize_with = "deserialize_duration"
  )]
  pub persistence: Duration,

  #[serde(default)]
  pub pool: Pool,

  #[serde(default)]
  pub forward: Vec<Node>,
}

const fn default_host() -> IpAddr {
  IpAddr::V4(Ipv4Addr::LOCALHOST)
}

const fn default_port() -> u16 {
  8080
}

const fn default_max_queue() -> usize {
  1024
}

const fn default_persistence() -> Duration {
  Duration::from_secs(300)
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
  D: Deserializer<'de>,
{
  let re = Regex::new(r"(?i)(\d+)(d(ays?)?|h(ours?)?|m(ins?)?|s(ec(onds?)?)?)")
    .map_err(de::Error::custom)?;
  let input = String::deserialize(deserializer)?;

  let (mut total_sec, mut total_len) = (0, 0);
  for cap in re.captures_iter(input.trim()) {
    let value = cap[1].parse::<u64>().map_err(de::Error::custom)?;
    match &cap[2].to_lowercase()[..] {
      "d" | "day" | "days" => total_sec += value * 86400,
      "h" | "hour" | "hours" => total_sec += value * 3600,
      "m" | "min" | "mins" => total_sec += value * 60,
      "s" | "sec" | "secs" => total_sec += value,
      _ => return Err(de::Error::custom("invalid unit")),
    }
    total_len += cap[0].len();
  }

  if input.chars().filter(|c| !c.is_whitespace()).count() == total_len {
    Ok(Duration::from_secs(total_sec))
  } else {
    Err(de::Error::custom("invalid format"))
  }
}

#[derive(Deserialize)]
pub struct Pool {
  pub threads: usize,
  pub brokers: usize,
}

impl Default for Pool {
  fn default() -> Self {
    Self {
      threads: 8,
      brokers: 4,
    }
  }
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
