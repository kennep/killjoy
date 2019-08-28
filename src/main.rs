// Keep the following in sync with the readme.
/*!
 * Monitor systemd units.
 *
 * killjoy is a systemd unit monitoring application. It discovers systemd units and tracks their
 * states. When a unit changes to a state of interest, killjoy generates notifications via
 * notifiers. Examples of notifiers include:
 *
 * * [killjoy Notifier: Logfile]
 * * [killjoy Notifier: Notification]
 *
 * Concepts
 * --------
 *
 * To understand killjoy, one must first understand systemd:
 *
 * > systemd is a suite of basic building blocks for a Linux system. It provides a system and
 * > service manager that runs as PID 1 and starts the rest of the system.
 * >
 * > — [systemd]
 *
 * Units are the resources that systemd knows how to manage. For example, here are several units
 * which might be present on a host, and the resources they represent:
 *
 * * `nginx.service`: Lightweight HTTP server and IMAP/POP3 proxy server
 * * `logrotate.service`: Rotate log files
 * * `logrotate.timer`: Rotate log files daily (i.e. periodically trigger `logrotate.service`)
 * * `boot.mount`: The `/boot` mount point
 *
 * This list of units is small; a host can have many hundreds of units, of eleven different types.
 *
 * There can be multiple systemd instances running on a host at a given time. Typically, there is
 * one system-wide instance, and one instance per logged-in user. Each systemd instance maintains
 * distinct units.
 *
 * When killjoy starts, it reads a list of rules, where each rule declares units that killjoy should
 * watch. For example, rules might state:
 *
 * * Connect to the system bus and watch `nginx.service`. If it enters the "failed" state, contact
 *   the "logfile" notifier.
 * * Connect to the session bus and watch all `.timer` units. If any enter the "active" state,
 *   contact the "notification" notifier.
 *
 * A notifier is an application that knows how to consume a D-Bus message from killjoy. The clear
 * separation between killjoy and the notifiers means that anyone may write a notifier at any time,
 * in whichever language they wish, to do whatever they want, and with no coordination from the
 * killjoy development team. Two notifiers are developed in conjunction with killjoy, and they are
 * small enough to be easily studied:
 *
 * * [killjoy Notifier: Logfile]
 * * [killjoy Notifier: Notification]
 *
 * For further conceptual information, see [systemd(1)], especially the section on [concepts].
 *
 * Alternatives
 * ------------
 *
 * killjoy is inspired by [sagbescheid], [SystemdMon], [pynagsystemd], and [`OnFailure=`], but there
 * are differences in features, reliability, and efficiency. Of special note:
 *
 * * killjoy lets a user write generic rules, like "monitor all `.timer` units." This is in contrast
 *   to the case where a user must explicitly state every unit to be monitored. Furthermore, units
 *   may appear or disappear at runtime, e.g. when a package is installed or uninstalled, and
 *   killjoy correctly handles these events.
 * * killjoy is cleanly separated from notifiers. Users aren't restricted to the notifiers bundled
 *   with killjoy.
 *
 * Installation
 * ------------
 *
 * Arch Linux users may install using the [killjoy-git] AUR package. A stable package will be
 * created when killjoy further matures.
 *
 * All other users may install killjoy from source. To do so:
 *
 * 1.   Ensure systemd, D-Bus, and the rust compiler are installed. On distributions which
 *      separately package libdbus, install that. (On Ubuntu, this is `libdbus-1-dev`.)
 * 2.   Get the source code, and compile and install it:
 *
 *      ```bash
 *      git clone https://github.com/Ichimonji10/killjoy.git
 *      cd killjoy
 *      scripts/install.sh
 *      ```
 *
 * Configuration
 * -------------
 *
 * Configuration files are searched for as per the [XDG Base Directory Specification]. The first one
 * found is used. A sample configuration file is as follows:
 *
 * ```json
 * {
 *     "version": 1,
 *     "rules": [
 *         {
 *             "bus_type": "session",
 *             "active_states": ["activating", "active", "deactivating", "inactive", "failed"],
 *             "expression": "foo.service",
 *             "expression_type": "unit name",
 *             "notifiers": ["logfile", "notification"]
 *         }
 *     ],
 *     "notifiers": {
 *         "logfile": {
 *             "bus_type": "session",
 *             "bus_name": "name.jerebear.KilljoyNotifierLogfile1"
 *         },
 *         "notification": {
 *             "bus_type": "session",
 *             "bus_name": "name.jerebear.KilljoyNotifierNotification1"
 *         }
 *     }
 * }
 * ```
 *
 * The contents of the settings file may be validated with `killjoy settings validate`.
 *
 * The meaning of the configuration file is as follows:
 *
 * *    `version` defines how the rest of the configuration file is interpreted. There is currently
 *      one configuration file format, and this key should always be set to 1.
 * *    `rules` is a list of rules stating which units should be monitored. For each rule:
 *      *   `bus_type` defines which D-Bus buses killjoy shall connect to in search of systemd
 *          instances. It may be `session` or `system`.
 *      *   All possible `active_states` are listed above; see [systemd(1)] for details.
 *      *   `expression_type` and `expression` define which units should be monitored (out of all
 *          the units killjoy discovers when talking to systemd). If `expression_type` is:
 *          *   `unit name`, then `expression` should be an exact unit name, like `foo.service`.
 *          *   `unit type`, then `expression` should be a unit suffix, like `.service`.
 *          *   `regex`, then `expression` should be a [regex] like `^f[aeiou]{2}\.service$`. Note
 *              the presence of the line begin and end anchors, `^` and `$`.
 *      *   `notifiers` is a list of notifier labels.
 * *    `notifiers` is a map, where keys are notifier labels, and values define how to contact that
 *      notifier.
 *      *   `bus_type` defines which message bus killjoy should connect to when sending a message to
 *          this notifier.
 *      *   `bus_name` defines the bus name (i.e. address) of the notifier on the message bus.
 *
 * Usage
 * -----
 *
 * The typical way to use killjoy is to let it automatically start on login:
 *
 * ```bash
 * systemctl --user enable --now killjoy.service
 * ```
 *
 * killjoy may also be invoked manually. Execute `killjoy` to run killjoy in the foreground, or
 * `killjoy --help` to learn about its features.
 *
 * License
 * -------
 *
 * killjoy is licensed under the GPLv3 or any later version.
 *
 * [SystemdMon]: https://github.com/joonty/systemd_mon
 * [XDG Base Directory Specification]: https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html
 * [`OnFailure=`]: https://www.freedesktop.org/software/systemd/man/systemd.unit.html
 * [concepts]: https://www.freedesktop.org/software/systemd/man/systemd.html#Concepts
 * [killjoy Notifier: Logfile]: https://github.com/Ichimonji10/killjoy-notifier-logfile
 * [killjoy Notifier: Notification]: https://github.com/Ichimonji10/killjoy-notifier-notification
 * [killjoy-git]: https://aur.archlinux.org/packages/killjoy-git/
 * [pynagsystemd]: https://github.com/kbytesys/pynagsystemd
 * [regex]: https://docs.rs/crate/regex/
 * [sagbescheid]: https://sagbescheid.readthedocs.io/en/latest/
 * [systemd(1)]: https://www.freedesktop.org/software/systemd/man/systemd.html
 * [systemd]: https://freedesktop.org/wiki/Software/systemd/
 */

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
