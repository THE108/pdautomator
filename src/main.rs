extern crate serde;
extern crate serde_json;
extern crate chrono;
extern crate futures;
extern crate hyper;
extern crate tokio_core;
extern crate hyper_tls;
extern crate getopts;
extern crate regex;

#[macro_use]
extern crate serde_derive;

mod pagerduty;
mod cmd;
mod config;

use std::env;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use getopts::Options;
use chrono::prelude::*;
use pagerduty::{Client, IncidentStatus};
use regex::{Regex, RegexSet};

#[derive(Debug)]
enum Error {
    PagerdutyError(pagerduty::Error),
    RegexError(regex::Error),
}

impl From<pagerduty::Error> for Error {
    fn from(error: pagerduty::Error) -> Self {
        Error::PagerdutyError(error)
    }
}

impl From<regex::Error> for Error {
    fn from(error: regex::Error) -> Self {
        Error::RegexError(error)
    }
}

fn get_regexes(cfg: &config::Config) -> Result<(RegexSet, Vec<Regex>), regex::Error> {
    let regex_strs: Vec<_> = cfg.actions.iter().map(|action| format!("^{}$", action.alert.trim())).collect();
    let set = RegexSet::new(&regex_strs)?;
    let mut regexes: Vec<Regex> = Vec::new();
    for r in regex_strs.iter() {
        regexes.push(Regex::new(r)?);
    }

    Ok((set, regexes))
}

fn get_commands_by_actions(cli: &mut Client, date: Date<Local>, cfg: &config::Config) -> Result<HashMap<config::Action, Vec<(String, String)>>, Error> {
    let incidents = cli.get_incidents(Some(date),
                                 None, Some(IncidentStatus::Triggered),
                                 vec!["id".to_string(), "trigger_summary_data".to_string()])?;

    let (set, regexes) = get_regexes(cfg)?;

    let mut cmd_by_action: HashMap<config::Action, Vec<(String, String)>> = HashMap::new();
    for incident in incidents {
        let desc = match incident.trigger_summary_data {
            Some(data) => data.description,
            None => continue,
        };

        let desc = match desc {
            Some(desc) => desc,
            None => continue,
        };

        let desc = desc.trim();

        let incident_id = match incident.id {
            Some(id) => id,
            None => continue,
        };

        println!("desc: {}", desc);

        for index in set.matches(desc).into_iter() {
            let action = match cfg.actions.get(index) {
                Some(action) => action.clone(),
                None => continue,
            };

            let command = match regexes.get(index) {
                Some(regexp) => regexp.replace_all(desc, &action.cmd as &str).to_string(),
                None => continue,
            };

            println!("action: {:?}", action);

            cmd_by_action.entry(action).or_insert(Vec::new()).push((incident_id.clone(), command));
        }
    }

    Ok(cmd_by_action)
}

fn resolve(action: &config::Action, pagerduty_cfg: &config::Pagerduty, incident_id: &str, stdout: &str) -> Result<(), Error> {
    if !action.resolve.unwrap_or(false) {
        return Ok(());
    }

    if let Some(ref resolve_check) = action.resolve_check {
        let re = Regex::new(resolve_check)?;

        if !re.is_match(stdout) {
            return Ok(());
        }
    }

    let mut cli = Client::new(&pagerduty_cfg.token, &pagerduty_cfg.org, &pagerduty_cfg.timezone, &pagerduty_cfg.timezone_short)?;

    cli.resolve(incident_id, &pagerduty_cfg.requester_id)?;

    Ok(())
}

fn print_usage(program: &str, opts: Options) {
    println!("{}: {:?}", program, opts.usage("pdautomator"));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("c", "config", "config file name", "CONFIG");
    opts.optflag("d", "debug", "print messages to console instead of slack");
    opts.optflag("h", "help", "print this help menu");
    let matches = opts.parse(&args[1..]).expect("couldn't parse command line");
    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    }

    let config_filename = matches.opt_str("c").unwrap_or(String::from("config.toml"));

    let cfg: config::Config = config::parse(&config_filename).expect(&format!("can't parse config '{}'", config_filename));

    println!("config: {:?}", cfg);

    let mut cli = Client::new(&cfg.pagerduty.token, &cfg.pagerduty.org, &cfg.pagerduty.timezone, &cfg.pagerduty.timezone_short).unwrap();

    let since = Local::now() - chrono::Duration::days(cfg.pagerduty.since_days.into());

    let commands_by_actions = get_commands_by_actions(&mut cli, since.date(), &cfg).unwrap();

    println!("cmds: {:?}", commands_by_actions);

    let mut workers = Vec::new();
    for (action, commands) in commands_by_actions {
        let pagerduty_cfg = cfg.pagerduty.clone();
        workers.push(thread::spawn(move || {
            for (incident_id, command) in commands {
                let (stdout, stderr) = match cmd::run(&command) {
                    Ok(result) => result,
                    Err(err) => {
                        println!("error: {}", err);
                        return;
                    }
                };

                println!("stdout: {}", stdout);
                println!("stderr: {}", stderr);

                let _ = resolve(&action, &pagerduty_cfg, &incident_id, &stdout)
                            .map_err(|err| println!("error: {:?}", err));

                if let Some(pause_sec) = action.pause_sec {
                    if pause_sec > 0 {
                        thread::sleep(Duration::from_secs(pause_sec));
                    }
                }
            }
        }));
    }

    for worker in workers {
        let _ = worker.join();
    }
}
