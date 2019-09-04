//! Monitor systemd units.
//!
//! See the readme for full documentation.

mod bus;
mod cli;
mod error;
mod generated;
mod settings;
mod timestamp;
mod unit;

use std::path::Path;
use std::process;
use std::thread;

use clap::ArgMatches;

use crate::bus::BusWatcher;
use crate::settings::Settings;

// The entry point for the application.
fn main() {
    let args = cli::get_cli_args();
    match args.subcommand() {
        ("settings", Some(sub_args)) => handle_settings_subcommand(&sub_args),
        _ => {
            let loop_once = args.is_present("loop-once");
            let loop_timeout = get_loop_timeout_or_exit(&args);
            handle_no_subcommand(loop_once, loop_timeout);
        }
    }
}

// Handle the 'settings' subcommand.
fn handle_settings_subcommand(args: &ArgMatches) {
    match args.subcommand() {
        ("load-path", Some(_)) => handle_settings_load_path_subcommand(),
        ("validate", Some(sub_args)) => handle_settings_validate_subcommand(&sub_args),
        _ => eprintln!("An unexpected code path executed. Please contact the developer."),
    }
}

// Handle the 'settings load-path' subcommand.
fn handle_settings_load_path_subcommand() {
    let load_path = settings::get_load_path().unwrap_or_else(|err| {
        eprintln!("{}", err);
        process::exit(1);
    });
    println!("{}", load_path.as_path().display());
}

// Handle the 'settings validate' subcommand.
fn handle_settings_validate_subcommand(args: &ArgMatches) {
    let path = args.value_of("path").map(|path_str| Path::new(path_str));
    get_settings_or_exit(path);
}

// Handle no subcommand at all.
//
// For each unique D-Bus bus listed in the settings file, spawn a thread. Each thread connects to a
// D-Bus bus, and talks to the instance of systemd available on that bus, and the notifiers
// available on that bus.
fn handle_no_subcommand(loop_once: bool, loop_timeout: u32) {
    let mut exit_code = 0;
    let settings: Settings = get_settings_or_exit(None);
    let handles: Vec<_> = settings::get_bus_types(&settings.rules)
        .into_iter()
        .map(|bus_type| {
            let settings_clone = settings.clone();
            thread::spawn(move || {
                BusWatcher::new(bus_type, settings_clone, loop_once, loop_timeout).run()
            })
        })
        .collect();

    // Handles are joined in the order they appear in the vector, not the order in which they exit,
    // meaning that there may be a long delay between an error occurring and this main thread
    // learning about it. Consequently, the monitoring threads should print their own error messages
    // whenever possible.
    for handle in handles {
        match handle.join() {
            Err(err) => eprintln!("Monitoring thread panicked. Error: {:?}", err),
            Ok(result) => {
                if result.is_err() {
                    exit_code = 1;
                }
            }
        }
    }
    process::exit(exit_code);
}

// Get and return a settings object, or print a message to stderr and exit with a non-zero code.
fn get_settings_or_exit(path: Option<&Path>) -> Settings {
    settings::load(path).unwrap_or_else(|err| {
        eprintln!("{}", err);
        process::exit(1);
    })
}

// Get the `loop-timeout` argument, or kill this process.
fn get_loop_timeout_or_exit(args: &ArgMatches) -> u32 {
    // It's safe to call expect(), because a default value is set in our arg parser.
    args.value_of("loop-timeout")
        .unwrap_or_else(|| {
            eprintln!(
                "Failed to get loop-timeout argument. Default should've been set in arg parser."
            );
            process::exit(1);
        })
        .parse::<u32>()
        .unwrap_or_else(|err| {
            eprintln!("Failed to parse argument loop-timeout: {}", err);
            process::exit(1);
        })
}
