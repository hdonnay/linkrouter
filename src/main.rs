use std::fs::File;
use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use clap::{App, Arg};
use dbus::{
    arg,
    blocking::{BlockingSender, Connection},
    Message,
};
use regex::{Regex, RegexSet};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use xdg::BaseDirectories;

fn main() -> Result<()> {
    let dirs = BaseDirectories::with_prefix("linkrouter").unwrap();
    let app = App::new("linkrouter")
        .after_help(LONGHELP)
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
    let dbus = Connection::new_session()?;

    let mut pats = Vec::new();
    let mut regexps = Vec::new();
    let mut rules = Vec::new();
    for v in dirs
        .list_config_files("")
        .into_iter()
        .filter(|f| f.extension().map_or(false, |v| v == "yaml"))
    {
        let f = File::open(v)?;
        let l: Vec<Rule> = serde_yaml::from_reader(f)?;
        for p in l {
            pats.push(p.pattern.clone());
            regexps.push(Regex::new(&p.pattern)?);
            rules.push(p);
        }
    }
    let set = RegexSet::new(&pats)?;

    for u in m.values_of("urls").unwrap() {
        let m = set.matches(u);
        if !m.matched_any() {
            eprintln!("no rule matched, using default command: {}", default_cmd);
            // default action
            match Command::new(default_cmd).arg(u).status()?.code() {
                Some(code) => {
                    if code != 0 {
                        println!("Exited with status code: {}", code)
                    }
                }
                None => println!("Process terminated by signal"),
            }
            continue;
        }
        let i = m.iter().next().expect("first index missing?");
        let p = &rules[i];
        let re = &regexps[i];
        if let Some(l) = &p.exec {
            let args: Vec<String> = l[1..]
                .iter()
                .map(|v| re.replace(u, v).into_owned())
                .collect();
            match Command::new(&l[0]).args(&args).status()?.code() {
                Some(code) => {
                    if code != 0 {
                        println!("Exited with status code: {}", code)
                    }
                }
                None => println!("Process terminated by signal"),
            }
        } else if let Some(d) = &p.dbus {
            eprintln!("Dbus support unfinished, this will crash.");
            let msg = Message::call_with_args(
                &d.destination,
                &d.path,
                &d.interface,
                &d.method,
                d.args()?,
            );
            dbus.send_with_reply_and_block(msg, Duration::from_secs(5))?;
        } else {
            return Err(Error::MatchBotch.into());
        }
    }
    Ok(())
}

#[derive(Deserialize, Serialize)]
struct Rule {
    pattern: String,
    exec: Option<Vec<String>>,
    dbus: Option<DbusRule>,
}

#[derive(Deserialize, Serialize)]
struct DbusRule {
    destination: String,
    path: String,
    interface: String,
    method: String,
    signature: String,
    args: Vec<serde_yaml::Value>,
}

impl DbusRule {
    fn args(&self) -> Result<DbusArgs> {
        Ok(DbusArgs {
            sig: dbus::Signature::new(&self.signature).map_err(|e| Error::Dbus(e))?,
            args: &self.args,
        })
    }
}

struct DbusArgs<'a> {
    sig: dbus::Signature<'a>,
    args: &'a [serde_yaml::Value],
}

impl arg::AppendAll for DbusArgs<'_> {
    fn append(&self, args: &mut arg::IterAppend) {
        use serde_yaml::Value;
        let mut i = 0;
        eprintln!("# signature: {}", self.sig);
        for c in self.sig.trim_matches(|c| c == '(' || c == ')').chars() {
            match c {
                'y'/* u8 */ => unimplemented!(),
                'b'/* bool */ => {
                    let v = &self.args[i];
                    if let Value::Bool(b) = v {
                        args.append(b)
                    } else {
                        panic!("wrong type at index {}", i)
                    }
                },
                'n'/* i16 */ => unimplemented!(),
                'q'/* u16 */ => unimplemented!(),
                'i'/* i32 */ => unimplemented!(),
                'u'/* u32 */ => 
                    // TODO Write a macro for these.
                    if let Value::Number(n) = &self.args[i] {
                        args.append(if let Some(i) = n.as_u64() {
                            i as u32
                        } else {
                            panic!("wrong type at index {}", i)
                        })
                    } else {
                        panic!("wrong type at index {}", i)
                    }
                'x'/* i64 */ => unimplemented!(),
                't'/* u64 */ => unimplemented!(),
                'd'/* f64 */ => unimplemented!(),
                'h'/* fd, aka u32 */ => unimplemented!(),
                's'/* String */ => {
                    let v = &self.args[i];
                    if let Value::String(s) = v {
                        args.append(s)
                    } else {
                        panic!("wrong type at index {}", i)
                    }
                },
                'o'/* object path */ => unimplemented!(),
                'g'/* signature */ => unimplemented!(),
                'a'/* array */ => {
                    i += 1;
                    // unwrap inner kind
                    eprintln!("array of {:?}", &self.args[i]);
                    unimplemented!()
                },
                '('/* struct */ => unimplemented!(),
                '{'/* dict element */ => unimplemented!(),
                'v'/* variant */ => unimplemented!(),
                _ => panic!("invalid"),
            }
            i += 1;
        }
        unimplemented!();
        /*
        for a in &d.args {
            match a {
                Value::Null => unimplemented!(),
                Value::Bool(b) => args.append(b),
                Value::Number(n) => {
                    if n.is_i64() {
                        args.append(n.as_i64().unwrap())
                    } else if n.is_u64() {
                        args.append(n.as_u64().unwrap())
                    } else {
                        args.append(n.as_f64().unwrap())
                    }
                }
                Value::String(s) => args.append(s),
                Value::Sequence(s) => args.append_array(
                    &dbus::Signature::new("as").map_err(|e| Error::Dbus(e))?,
                    |arr| {
                        for e in s {
                            match e {
                                Value::String(s) => arr.append(s),
                                _ => unimplemented!(),
                            }
                        }
                    },
                ),
                Value::Mapping(_m) => unimplemented!(),
            }
        }
        */
    }
}

#[derive(Error, Debug)]
enum Error {
    #[error("found match but no action")]
    MatchBotch,
    #[error("dbus error: {}", .0)]
    Dbus(String),
}

static LONGHELP: &str = r#"Linkrouter looks for any files matching "*.yaml" in $XDG_CONFIG_HOME/linkrouter and expects them to
contain contain arrays of rules.

A basic rule file looks like:

    - pattern: ^https?://
      exec:
      - x-www-browser
      - $0
"#;
