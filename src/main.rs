use std::fs::File;
use std::process::Command;

use anyhow::Result;
use clap::{App, Arg};
use regex::{Regex, RegexSet};
use serde::{Deserialize, Serialize};
use xdg::BaseDirectories;

fn main() -> Result<()> {
    let dirs = BaseDirectories::with_prefix("linkrouter").unwrap();
    let app = App::new("linkrouter")
        .arg(Arg::with_name("debug").short("D").help(""))
        .arg(
            Arg::with_name("command")
                .long("cmd")
                .help("")
                .default_value("xdg-open"),
        )
        .arg(Arg::with_name("urls").index(1).multiple(true));
    let m = app.get_matches();
    let _debug = m.is_present("debug");
    let default_cmd = m.value_of("command").expect("???");

    let mut pats = Vec::new();
    let mut regexps = Vec::new();
    let mut pairs = Vec::new();
    for v in dirs
        .list_config_files("")
        .into_iter()
        .filter(|f| f.extension().map_or(false, |v| v == "yaml"))
    {
        let f = File::open(v)?;
        let l: Vec<Pair> = serde_yaml::from_reader(f)?;
        for p in l {
            pats.push(p.pattern.clone());
            regexps.push(Regex::new(&p.pattern)?);
            pairs.push(p);
        }
    }
    let set = RegexSet::new(&pats)?;

    for u in m.values_of("urls").unwrap() {
        let m = set.matches(u);
        let status = if !m.matched_any() {
            // default action
            Command::new(default_cmd).arg(u).status()?
        } else {
            let i = m.iter().next().expect("first index missing?");
            let p = &pairs[i];
            let re = &regexps[i];
            let args: Vec<String> = p
                .args
                .iter()
                .map(|v| re.replace(u, v).into_owned())
                .collect();
            Command::new(&p.command).args(&args).status()?
        };
        match status.code() {
            Some(code) => {
                if code != 0 {
                    println!("Exited with status code: {}", code)
                }
            }
            None => println!("Process terminated by signal"),
        }
    }
    Ok(())
}

#[derive(Deserialize, Serialize)]
struct Pair {
    pattern: String,
    command: String,
    args: Vec<String>,
}
