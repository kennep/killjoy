//! Tools for working with the killjoy systemd unit monitoring application.

mod bus;
pub mod cli;
mod error;
mod generated;
pub mod settings;
mod unit;

use std::collections::HashSet;
use std::thread;

use bus::BusWatcher;
use settings::Settings;

// How verbose should error messages be?
const VERBOSE: bool = false;

// Human-readable names for D-Bus buses.
const SESSION_BUS_PROSE_NAME: &'static str = "session";
const SYSTEM_BUS_PROSE_NAME: &'static str = "system";

// Get all of the D-Bus bus types listed in the given settings object.
fn get_bus_types(settings: &Settings) -> Vec<dbus::BusType> {
    settings
        .rules
        .iter()
        .map(|rule| &rule.bus[..])
        .collect::<HashSet<&str>>()
        .into_iter()
        .filter_map(|bus| match bus {
            SESSION_BUS_PROSE_NAME => Some(dbus::BusType::Session),
            SYSTEM_BUS_PROSE_NAME => Some(dbus::BusType::System),
            _ => None,
        })
        .collect()
}

/// Connect to D-Bus buses, and maintain state machines for relevant units.
///
/// For each D-Bus bus listed in the settings argument, spawn and start a thread, and configure that
/// thread to connect to one of the buses. At a high level, a thread monitors the interesting units
/// accessible via that bus' systemd instance, and takes action when a unit enters an interesting
/// state.
///
/// Whether a unit is an "interesting unit," and whether it is entering an "interesting state," is
/// defined by the rules in the settings file. Currently, taking action consists of printing a
/// debugging message to the console. In the future, this will consist of reaching out across the
/// D-Bus and contacting the appropriate notifier.
pub fn run(settings: &Settings) {
    let handles: Vec<_> = get_bus_types(settings)
        .into_iter()
        .map(|bus_type| {
            let settings_clone = settings.clone();
            thread::spawn(move || BusWatcher::new(bus_type, settings_clone).run())
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}

#[cfg(test)]
mod test_utils {
    use crate::settings::Rule;
    use crate::{SESSION_BUS_PROSE_NAME, SYSTEM_BUS_PROSE_NAME};

    pub fn gen_session_rule() -> Rule {
        Rule {
            active_states: Vec::new(),
            bus: SESSION_BUS_PROSE_NAME.to_owned(),
            expression: "".to_owned(),
            expression_type: "".to_owned(),
            notifiers: Vec::new(),
        }
    }

    pub fn gen_system_rule() -> Rule {
        Rule {
            active_states: Vec::new(),
            bus: SYSTEM_BUS_PROSE_NAME.to_owned(),
            expression: "".to_owned(),
            expression_type: "".to_owned(),
            notifiers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_get_bus_types_v1() {
        let settings = Settings {
            notifiers: HashMap::new(),
            rules: Vec::new(),
            version: 1,
        };
        let bus_types = get_bus_types(&settings);
        assert!(!bus_types.contains(&dbus::BusType::Session));
        assert!(!bus_types.contains(&dbus::BusType::System));
    }

    #[test]
    fn test_get_bus_types_v2() {
        let settings = Settings {
            notifiers: HashMap::new(),
            rules: vec![test_utils::gen_session_rule()],
            version: 1,
        };
        let bus_types: Vec<dbus::BusType> = get_bus_types(&settings);
        assert!(bus_types.contains(&dbus::BusType::Session));
        assert!(!bus_types.contains(&dbus::BusType::System));
    }

    #[test]
    fn test_get_bus_types_v3() {
        let settings = Settings {
            notifiers: HashMap::new(),
            rules: vec![test_utils::gen_system_rule()],
            version: 1,
        };
        let bus_types: Vec<dbus::BusType> = get_bus_types(&settings);
        assert!(!bus_types.contains(&dbus::BusType::Session));
        assert!(bus_types.contains(&dbus::BusType::System));
    }

    #[test]
    fn test_get_bus_types_v4() {
        let settings = Settings {
            notifiers: HashMap::new(),
            rules: vec![
                test_utils::gen_session_rule(),
                test_utils::gen_system_rule(),
            ],
            version: 1,
        };
        let bus_types: Vec<dbus::BusType> = get_bus_types(&settings);
        assert!(bus_types.contains(&dbus::BusType::Session));
        assert!(bus_types.contains(&dbus::BusType::System));
    }
}
