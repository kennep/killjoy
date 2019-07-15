use std::process;

use clap::ArgMatches;

use killjoy::{cli, settings, settings::Settings};

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
        ("validate", Some(_)) => handle_settings_validate_subcommand(),
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
    println!("{}", load_path);
}

// Handle the 'settings validate' subcommand.
fn handle_settings_validate_subcommand() {
    get_settings_or_exit();
}

// Handle no subcommand at all.
fn handle_no_subcommand() {
    killjoy::run(&get_settings_or_exit());
}

// Get and return a settings object, or print a message to stderr and exit with a non-zero code.
fn get_settings_or_exit() -> Settings {
    match settings::load() {
        Ok(settings_obj) => settings_obj,
        Err(err) => {
            eprintln!("{}", err);
            process::exit(1);
        }
    }
}
