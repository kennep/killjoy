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

use std::path::{Path, PathBuf};
use std::process;
use std::thread;
use std::thread::JoinHandle;

use clap::ArgMatches;

use crate::bus::BusWatcher;
use crate::error::{SettingsFileError, TopLevelError};
use crate::settings::Settings;

// The entry point for the application.
fn main() {
    if let Err(errs) = handle_args() {
        for err in errs {
            eprintln!("{}", err);
        }
        process::exit(1);
    }
}

// Fetch and handle CLI arguments. On error may be returned per thread.
fn handle_args() -> Result<(), Vec<TopLevelError>> {
    let args = cli::get_cli_args();
    match args.subcommand() {
        ("settings", Some(sub_args)) => {
            handle_settings_subcommand(&sub_args).map_err(|err| vec![err])?
        }
        _ => {
            let loop_once = args.is_present("loop-once");
            let loop_timeout = get_loop_timeout(&args).map_err(|err| vec![err])?;
            handle_no_subcommand(loop_once, loop_timeout)?;
        }
    };
    Ok(())
}

// Handle the 'settings' subcommand.
fn handle_settings_subcommand(args: &ArgMatches) -> Result<(), TopLevelError> {
    match args.subcommand() {
        ("load-path", Some(_)) => handle_settings_load_path_subcommand(),
        ("validate", Some(sub_args)) => handle_settings_validate_subcommand(&sub_args),
        _ => Err(TopLevelError::UnexpectedSubcommand(
            args.subcommand_name().map(String::from),
        )),
    }?;
    Ok(())
}

// Handle the 'settings load-path' subcommand.
fn handle_settings_load_path_subcommand() -> Result<(), TopLevelError> {
    let load_path: PathBuf = settings::get_load_path().map_err(TopLevelError::SettingsFileError)?;
    println!("{}", load_path.as_path().display());
    Ok(())
}

// Handle the 'settings validate' subcommand.
fn handle_settings_validate_subcommand(args: &ArgMatches) -> Result<(), TopLevelError> {
    let path = args.value_of("path").map(|path_str| Path::new(path_str));
    settings::load(path).map_err(TopLevelError::SettingsFileError)?;
    Ok(())
}

// Handle no subcommand at all.
//
// For each unique D-Bus bus listed in the settings file, spawn a thread. Each thread connects to a
// D-Bus bus, and talks to the instance of systemd available on that bus, and the notifiers
// available on that bus.
fn handle_no_subcommand(loop_once: bool, loop_timeout: u32) -> Result<(), Vec<TopLevelError>> {
    let settings: Settings = settings::load(None)
        .map_err(|err: SettingsFileError| vec![TopLevelError::SettingsFileError(err)])?;
    let handles: Vec<JoinHandle<_>> = settings::get_bus_types(&settings.rules)
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
    let mut errs: Vec<TopLevelError> = Vec::new();
    for handle in handles {
        match handle.join() {
            Err(err) => errs.push(TopLevelError::MonitoringThreadPanicked(err)),
            Ok(result) => {
                if let Err(err) = result {
                    errs.push(TopLevelError::DBusError(err));
                }
            }
        }
    }
    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

// Get the `loop-timeout` argument, or return an error explaining why the getting failed.
fn get_loop_timeout(args: &ArgMatches) -> Result<u32, TopLevelError> {
    let loop_timeout: u32 = args
        .value_of("loop-timeout")
        .ok_or(TopLevelError::GetLoopTimeoutArg)?
        .parse::<u32>()
        .map_err(TopLevelError::ParseLoopTimeoutArg)?;
    Ok(loop_timeout)
}
