//! wallbar: desktop wallpapers from the macOS menu bar.
//!
//! One binary. With no arguments it is the menu bar app ([`app`]); with a
//! subcommand it is the CLI ([`cli`]) that `docs/cli.md` specifies and the
//! Raycast extension talks to.

mod app;
mod cli;
mod controller;
mod login;
mod menu_bar_icon;
mod status_item;
mod thumbs;
mod ui;

use std::process::ExitCode;

fn main() -> ExitCode {
    // Launch Services used to pass a -psn_ argument to apps it opened; ignore
    // it so an old-style launch still starts the app rather than the CLI.
    let args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with("-psn_"))
        .collect();
    if args.is_empty() {
        app::run();
        ExitCode::SUCCESS
    } else {
        cli::run(&args)
    }
}
