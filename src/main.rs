// Keep the following in sync with the readme.
/*!
 * Killjoy is a [systemd] unit monitoring application.
 *
 * What is systemd?
 *
 * > systemd is a suite of basic building blocks for a Linux system. It provides a system and
 * > service manager that runs as PID 1 and starts the rest of the system.
 *
 * Units are the resources that systemd knows how to manage. For example, the unit corresponding to
 * the nginx web server might be `nginx.service`, and the unit corresponding to the `/boot` mount
 * point might be `boot.mount`, though naming can vary per Linux distribution.
 *
 * Killjoy watches for a configurable list of events, such as "`nginx.service` failed," or
 * "`my-backup.service` is activating, active, or deactivating." Killjoy responds to these events by
 * reaching out across a D-Bus and contacting a configurable list of notifiers. In turn, the
 * notifiers are responsible for generating desktop pop-ups, sending emails, or otherwise taking
 * action.
 *
 * A small number of notifiers are developed alongside killjoy. However, the clear separation
 * between the watcher (killjoy) and the notifiers means that anyone can write and distribute a
 * custom notifier at any time, with no changes to killjoy itself. Want to start the WiFi coffee
 * maker when the daily backup service kicks off? Go for it.
 *
 * Killjoy is inspired by [sagbescheid], [SystemdMon], [pynagsystemd], and [`OnFailure=`], but there
 * are differences in efficiency, reliability, features, and flexibility. Killjoy assumes knowledge
 * of [systemd]. For additional information, see [systemd(1)], especially the section on [concepts].
 *
 * Dependencies
 * ------------
 *
 * Most dependencies used by Killjoy are pure Rust libraries and are listed in `Cargo.toml`.
 * However, Killjoy indirectly requires libdbus at runtime. (On Ubuntu, install `libdbus-1-dev`.)
 * For details, see the Rust dbus library's [requirements].
 *
 * License
 * -------
 *
 * Killjoy is licensed under the GPLv3 or any later version.
 *
 * [SystemdMon]: https://github.com/joonty/systemd_mon
 * [`OnFailure=`]: https://www.freedesktop.org/software/systemd/man/systemd.unit.html
 * [concepts]: https://www.freedesktop.org/software/systemd/man/systemd.html#Concepts
 * [pynagsystemd]: https://github.com/kbytesys/pynagsystemd
 * [requirements]: https://github.com/diwic/dbus-rs#requirements
 * [sagbescheid]: https://sagbescheid.readthedocs.io/en/latest/
 * [systemd(1)]: https://www.freedesktop.org/software/systemd/man/systemd.html
 * [systemd]: https://freedesktop.org/wiki/Software/systemd/
 */

mod bus;
mod cli;
mod error;
mod generated;
mod settings;
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
        _ => handle_no_subcommand(),
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
    let load_path = match settings::get_load_path() {
        Ok(load_path) => load_path,
        Err(err) => {
            eprintln!("{}", err);
            process::exit(1);
        }
    };
    println!("{}", load_path.as_path().display());
}

// Handle the 'settings validate' subcommand.
fn handle_settings_validate_subcommand(args: &ArgMatches) {
    let path = match args.value_of("path") {
        Some(path_str) => Some(Path::new(path_str)),
        None => None,
    };
    get_settings_or_exit(path);
}

// Handle no subcommand at all.
//
// For each unique D-Bus bus listed in the settings file, spawn a thread. Each thread connects to a
// D-Bus bus, and talks to the instance of systemd available on that bus, and the notifiers
// available on that bus.
fn handle_no_subcommand() {
    let mut exit_code = 0;
    let settings: Settings = get_settings_or_exit(None);
    let handles: Vec<_> = settings::get_bus_types(&settings.rules)
        .into_iter()
        .map(|bus_type| {
            let settings_clone = settings.clone();
            thread::spawn(move || BusWatcher::new(bus_type, settings_clone).run())
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
                if let Err(code) = result {
                    exit_code = code;
                }
            }
        }
    }
    process::exit(exit_code);
}

// Get and return a settings object, or print a message to stderr and exit with a non-zero code.
fn get_settings_or_exit(path: Option<&Path>) -> Settings {
    match settings::load(path) {
        Ok(settings_obj) => settings_obj,
        Err(err) => {
            eprintln!("{}", err);
            process::exit(1);
        }
    }
}
