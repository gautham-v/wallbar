//! The CLI half of the binary: `wallbar <command>`. See `docs/cli.md`.

use std::process::ExitCode;

use anyhow::{bail, Result};
use objc2::MainThreadMarker;
use wallbar_core::macos::MacDesktop;
use wallbar_core::wallbar::ClockRandom;
use wallbar_core::{Painting, Wallbar};

const USAGE: &str = "\
usage: wallbar                      run the menu bar app
       wallbar current [--json]
       wallbar list [--json]
       wallbar next [--all] [--json]
       wallbar prev [--all] [--json]
       wallbar shuffle [--all] [--json]
       wallbar set <filename|path> [--json]
       wallbar match-appearance on|off

Folder: $WALLBAR_DIR, default ~/Pictures/wallpapers";

/// Parsed flags common to the commands.
#[derive(Debug, Default, PartialEq)]
struct Flags {
    json: bool,
    all: bool,
    args: Vec<String>,
}

fn parse(rest: &[String], allow_all: bool) -> Result<Flags> {
    let mut flags = Flags::default();
    for arg in rest {
        match arg.as_str() {
            "--json" => flags.json = true,
            "--all" if allow_all => flags.all = true,
            s if s.starts_with("--") => bail!("unknown option {s}"),
            s => flags.args.push(s.to_owned()),
        }
    }
    Ok(flags)
}

/// Run a subcommand. `args` excludes the program name and is non-empty.
pub fn run(args: &[String]) -> ExitCode {
    let command = args[0].as_str();
    if matches!(command, "-h" | "--help" | "help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("wallbar: must run on the main thread");
        return ExitCode::FAILURE;
    };
    let wallbar = Wallbar::new(
        wallbar_core::default_dir(),
        wallbar_core::default_state_path(),
        MacDesktop::new(mtm),
    );
    match dispatch(&wallbar, command, &args[1..]) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) if err.is::<UsageError>() => {
            eprintln!("wallbar: {err}\n\n{USAGE}");
            ExitCode::from(2)
        }
        Err(err) => {
            eprintln!("wallbar: {err:#}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug)]
struct UsageError(String);
impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for UsageError {}

fn usage(msg: impl Into<String>) -> anyhow::Error {
    UsageError(msg.into()).into()
}

fn dispatch(w: &Wallbar<MacDesktop>, command: &str, rest: &[String]) -> Result<()> {
    let pool_command = matches!(command, "current" | "next" | "prev" | "shuffle");
    let flags = parse(rest, pool_command).map_err(|e| usage(e.to_string()))?;
    let no_args = |flags: &Flags| {
        if flags.args.is_empty() {
            Ok(())
        } else {
            Err(usage(format!("unexpected argument {}", flags.args[0])))
        }
    };
    match command {
        "current" => {
            no_args(&flags)?;
            print_painting(&w.current(flags.all)?, flags.json)
        }
        "next" => {
            no_args(&flags)?;
            print_painting(&w.next(flags.all)?, flags.json)
        }
        "prev" | "previous" => {
            no_args(&flags)?;
            print_painting(&w.prev(flags.all)?, flags.json)
        }
        "shuffle" => {
            no_args(&flags)?;
            print_painting(&w.shuffle(flags.all, &mut ClockRandom::new())?, flags.json)
        }
        "set" => {
            let [target] = flags.args.as_slice() else {
                return Err(usage("set takes one filename or path"));
            };
            print_painting(&w.set(target)?, flags.json)
        }
        "list" => {
            no_args(&flags)?;
            let listing = w.list()?;
            if flags.json {
                println!("{}", serde_json::to_string(&listing)?);
            } else {
                for p in &listing.items {
                    let mark = if listing.current.as_deref() == Some(p.file.as_str()) {
                        "*"
                    } else {
                        " "
                    };
                    println!("{mark} {}", p.one_line());
                }
            }
            Ok(())
        }
        "match-appearance" => {
            let on = match flags.args.as_slice() {
                [v] if v == "on" => true,
                [v] if v == "off" => false,
                _ => return Err(usage("match-appearance takes on or off")),
            };
            w.set_match_appearance(on)?;
            if flags.json {
                println!("{}", serde_json::json!({ "match_appearance": on }));
            } else {
                println!("Match appearance: {}", if on { "on" } else { "off" });
            }
            Ok(())
        }
        other => Err(usage(format!("unknown command {other}"))),
    }
}

fn print_painting(p: &Painting, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(p)?);
    } else {
        println!("{}", p.one_line());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn flags_parse_in_any_order() {
        let f = parse(&strings(&["--json", "--all"]), true).unwrap();
        assert!(f.json && f.all && f.args.is_empty());
        let f = parse(&strings(&["x.jpg", "--json"]), false).unwrap();
        assert_eq!(f.args, vec!["x.jpg"]);
        assert!(parse(&strings(&["--all"]), false).is_err());
        assert!(parse(&strings(&["--bogus"]), true).is_err());
    }
}
