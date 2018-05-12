extern crate toml;
extern crate failure;

use std::fs::File;
use std::io::Read;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub pagerduty: Pagerduty,
    pub actions: Vec<Action>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Pagerduty {
    pub org: String,
    pub token: String,
    pub timezone: String,
    pub timezone_short: String,
    pub fetch_interval_sec: u32,
    pub since_days: u32,
    pub requester_id: String,
}

#[derive(Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub struct Action {
    pub alert: String,
    pub cmd: String,
    pub pause_sec: Option<u64>,
    pub resolve: Option<bool>,
    pub resolve_check: Option<String>,
}

pub fn parse(filename: &str) -> Result<Config, failure::Error> {
    let mut fd = File::open(filename)?;

    let mut contents = String::new();
    fd.read_to_string(&mut contents)?;

    let config: Config = toml::from_str(&contents)?;

    Ok(config)
}
