//! Logic for interacting with the CLI.

use clap;
use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use regex::Regex;

/// Consume CLI arguments, parse them, validate them, and return the digested result.
pub fn get_cli_args<'a>() -> ArgMatches<'a> {
    let help_messages = HelpMessagesFactory::new().gen_help_messages();
    App::new("killjoy")
        .version(clap::crate_version!())
        .author("Jeremy Audet <jerebear@protonmail.com>")
        .about("Monitor systemd units.")
        .max_term_width(100)
        .subcommand(
            SubCommand::with_name("settings")
                .about("Manage the settings file.")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("load-path")
                        .about("Print the path to the file from which settings are loaded.")
                        .after_help(&help_messages.settings_load_path[..]),
                )
                .subcommand(
                    SubCommand::with_name("validate")
                        .about("Validate the settings file.")
                        .after_help(&help_messages.settings_validate[..])
                        .arg(
                            Arg::with_name("path")
                                .help("The path to the settings file to validate."),
                        ),
                ),
        )
        .get_matches()
}

// Help messages for use by a CLI parser.
struct HelpMessages {
    settings_load_path: String,
    settings_validate: String,
}

// A factory for generating `HelpMessages` structs.
struct HelpMessagesFactory {
    re: Regex,
}

impl HelpMessagesFactory {
    // Create a new factory.
    //
    // Creating a factory can be an expensive operation, as it involves compiling a regex.
    fn new() -> Self {
        let re = Self::get_re();
        HelpMessagesFactory { re }
    }

    // Create a struct containing help messages formatted for the current terminal.
    fn gen_help_messages(&self) -> HelpMessages {
        let settings_load_path = self.format(Self::get_help_for_settings_load_path());
        let settings_validate = self.format(Self::get_help_for_settings_validate());
        HelpMessages {
            settings_load_path,
            settings_validate,
        }
    }

    // ---------------------------------------------------------------------------------------------
    // End porcelain methods, start plumbing methods.
    // ---------------------------------------------------------------------------------------------

    // Format an unformatted help message.
    fn format(&self, msg: &str) -> String {
        let msg: String = textwrap::dedent(msg);
        let msg: std::borrow::Cow<str> = self.re.replace_all(msg.trim(), "$pre $post");
        msg.into_owned()
    }

    // Compile a RE that strips intra-paragraph newlines. Given this text:
    //
    //      one two
    //      three four
    //
    //      five six
    //
    // One can use the RE's replace_all() to produce this:
    //
    //      one two three four
    //
    //      five six
    //
    fn get_re() -> Regex {
        Regex::new(r"(?P<pre>\S)\n(?P<post>\S)").expect("Failed to compile regex.")
    }

    // Return the unformatted help message for the `settings load-path` subcommand.
    fn get_help_for_settings_load_path() -> &'static str {
        r###"
        Search an ordered list of directories for a settings file. If one is found, print its path.
        Otherwise, return a non-zero exit code. The load path is used by sibling commands such as
        "validate".
        "###
    }

    // Return the unformatted help message for the `settings validate` subcommand.
    fn get_help_for_settings_validate() -> &'static str {
        r###"
        Check to see whether the settings file conforms with a schema. If so, silently exit.
        Otherwise, print an error message to stderr and return non-zero.
        "###
    }
}
