use std::process;

use killjoy::{cli, settings};

fn main() {
    let args = cli::get_cli_args();
    match args.subcommand() {
        ("settings", Some(args_settings)) => match args_settings.subcommand() {
            ("load-path", Some(_)) => {
                let load_path = match settings::get_load_path() {
                    Ok(load_path) => load_path,
                    Err(err) => {
                        eprintln!("{}", err);
                        process::exit(1);
                    }
                };
                println!("{}", load_path);
            }
            ("validate", Some(_)) => {
                println!("Handling validate subcommand");
            }
            _ => {
                eprintln!("An unexpected code path executed. Please contact the developer.");
            }
        },
        _ => {
            let settings_obj = match settings::load() {
                Ok(settings_obj) => settings_obj,
                Err(err) => {
                    eprintln!("Failed to read configuration file: {}", err);
                    process::exit(1);
                }
            };
            killjoy::run(&settings_obj);
        }
    }
}
