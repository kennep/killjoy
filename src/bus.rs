// Logic for interacting with D-Bus buses.

use std::collections::HashMap;
use std::convert::TryFrom;

use dbus::arg::{RefArg, Variant};
use dbus::{
    BusName, BusType, ConnPath, Connection, Error as DBusError, Interface, Member, Message, Path,
    SignalArgs,
};

use crate::error::DBusError as CrateDBusError;
use crate::generated::org_freedesktop_systemd1::OrgFreedesktopDBusProperties;
use crate::generated::org_freedesktop_systemd1::OrgFreedesktopDBusPropertiesPropertiesChanged as PropertiesChanged;
use crate::generated::org_freedesktop_systemd1::OrgFreedesktopSystemd1Manager;
use crate::generated::org_freedesktop_systemd1::OrgFreedesktopSystemd1ManagerUnitNew as UnitNew;
use crate::generated::org_freedesktop_systemd1::OrgFreedesktopSystemd1ManagerUnitRemoved as UnitRemoved;
use crate::settings::{Rule, Settings};
use crate::unit::{ActiveState, UnitStateMachine};

const BUS_NAME_FOR_SYSTEMD: &str = "org.freedesktop.systemd1";
const PATH_FOR_SYSTEMD: &str = "/org/freedesktop/systemd1";
const INTERFACE_FOR_SYSTEMD_UNIT: &str = "org.freedesktop.systemd1.Unit";

// A unit's properties, as returned by a PropertiesChanged signal, or a call to
// org.freedesktop.systemd1.Unit.GetAll.
type UnitProps = HashMap<String, Variant<Box<dyn RefArg + 'static>>>;

// Watch units appear and disappear on a bus, and take actions in response.
pub struct BusWatcher {
    loop_once: bool,
    loop_timeout: u32,
    connection: Connection,
    settings: Settings,
}

impl BusWatcher {
    // Initialize a new monitor, but do not start watching units.
    //
    // To watch for units of interest, and to take action when those units of interest transition to
    // states of interest, call `run`.
    pub fn new(bus_type: BusType, settings: Settings, loop_once: bool, loop_timeout: u32) -> Self {
        let connection = Connection::get_private(bus_type)
            .expect(&format!("Failed to connect to {:?} D-Bus bus.", bus_type)[..]);
        let settings = settings;
        BusWatcher {
            loop_once,
            loop_timeout,
            connection,
            settings,
        }
    }

    // Track units of interest.
    //
    // Do the following:
    //
    // 1.  Subscribe to the `UnitRemoved` and `UnitNew` signals.
    // 2.  List extant units. For each interesting unit:
    //
    //     1.  Create a state machine for that unit.
    //     2.  Subscribe to the `PropertiesChanged` signal for that unit.
    //     3.  Get the unit's current state, and update the corresponding state machine.
    //
    // 3.  Infinitely process signals:
    //
    //     *   `UnitRemoved`: Delete the corresponding state machine, if it exists.
    //     *   `UnitNew`: If the unit is interesting, do the same as step 2, above.
    //     *   `PropertiesChanged`: Get the unit's current state, and update the corresponding
    //         state machine.
    //
    // An "interesting" unit is one that matches any of the monitoring rules provided by the user.
    //
    // Ordering matters. If the first two steps are swapped, then killjoy's behaviour could become
    // degenerate: it could miss units which appear while the list of extant units is being
    // processed.
    //
    // The ordering of step 2 also matters. If steps 2.2 and 2.3 are swapped, then killjoy's state
    // machines could fail to reflect reality:
    //
    // 1.   Killjoy asks for the state of a unit, and finds that it's OK.
    // 2.   The unit changes state. (Killjoy would not receive the signal.)
    // 3.   Killjoy subscribes to the `PropertiesChanged` signal for that unit.
    //
    // ----
    //
    // Remember that while D-Bus retains message ordering between peers, peers may send messages in
    // arbitrary order. If killjoy assumes that the order in which messages are received matches
    // the order in which events occurred, then its state machines can fail to reflect reality.
    // Consider the following example:
    //
    // 1.  Killjoy asks for the state of `foo.unittype`. Systemd receives the request and creates a
    //     response indicating that the unit is activating.
    // 2.  `foo.unittype` changes to the active state, and systemd emits a `PropertiesChanged`
    //     signal for that unit, where the signal includes the new unit state, "active." (The
    //     `changed_properties` attribute contains this information.)
    // 3.  Systemd sends the response created in step 1, where the response indicates that
    //     `foo.unittype` is activating.
    //
    // If killjoy naively assumes that message ordering reflects event ordering, then the state
    // machine for `foo.unittype` will end up with a state of "activating."
    //
    // One can resolve this issue by using timestamps. First, whenever getting the state of a unit,
    // make sure to also fetch the timestamp indicating when that state change occurred, and store
    // both the state and timestamp in the state machine. (`org.freedesktop.systemd1.Unit` has
    // several `*Timestamp*` properties.) Second, only update the state machine if the timestamp
    // retrieved from systemd is newer than the timestamp on the state machine. As applied to the
    // example above, these rules would ensure that the state machine for `foo.unittype` stays in
    // the "active" state, and does not transition to the "activating" state.
    //
    // Notice that killjoy's state machine for `foo.unittype` skips a transition. In most contexts,
    // for a state machine to skip a transition is a bug, as it means that the user-provided list
    // of rules won't be examined for a potential event of interest. However, if one assumes that
    // `PropertiesChanged` signals for a unit are emitted in the same order as underlying state
    // changes, then this skip can only occur during startup, when killjoy is both explicitly
    // requesting unit states and consuming `PropertiesChanged` signals. To skip state transitions
    // during start-up is non-ideal but acceptable, as prior to startup, units were not being
    // monitored anyway.
    //
    // Given all of the above, we can make the following statement:
    //
    // > With two caveats, all state changes for "interesting" units will be handled by killjoy.
    //
    // The caveats are:
    //
    // #.  When a unit is loaded into memory by systemd, there is a period of time during which
    //     killjoy will miss state changes.
    // #.  Killjoy may miss state changes that occur before startup is complete.
    //
    // Startup is complete when all unicast messages requesting unit states have been received a
    // response and been processed. After that point, all `PropertiesChanged` signals are either
    // out-of-date and discarded, or newer and useful.
    pub fn run(&self) -> Result<(), CrateDBusError> {
        self.call_manager_subscribe()?;

        // D-Bus inserts a org.freedesktop.DBus.NameAcquired signal into the message queue of new
        // connections. Discard it before subscribing to any other signals.
        self.connection.incoming(1000).next();

        // It's important to subscribe to UnitRemoved before UnitNew. Doing so prevents the
        // following scenario:
        //
        // 1.   A connection is subscribed to UnitNew announcements.
        // 2.   Systemd announces UnitNew for foo.unittype, which the connection receives.
        // 3.   Systemd announces UnitRemoved for foo.unittype, which the connection misses.
        // 4.   A connection is subscribed to UnitRemoved announcements.
        //
        // In this scenario, killjoy would consume the announcements queued up at the connection,
        // and incorrectly conclude that foo.unittype is present.
        self.subscribe_manager_unit_removed()?;
        self.subscribe_manager_unit_new()?;

        // Learn about interesting extant units. If any calls to systemd fail, assume the unit has
        // been unloaded and a UnitRemoved signal has been broadcast. The UnitRemoved handler should
        // clean up the subscription to PropertiesChanged for that unit, if any.
        let mut unit_states: HashMap<String, UnitStateMachine> = HashMap::new();
        {
            let borrowed_rules: Vec<&Rule> = self.settings.rules.iter().collect();
            let unit_names: Vec<String> = self.call_manager_list_units()?;
            for unit_name in unit_names {
                if rules_match_name(&borrowed_rules, &unit_name) {
                    let unit_path = match self.call_manager_get_unit(&unit_name) {
                        Ok(unit_path) => unit_path,
                        Err(_) => continue,
                    };
                    self.subscribe_properties_changed(&unit_path)?;
                    let unit_props = match self.call_properties_get_all(&unit_path) {
                        Ok(unit_props) => unit_props,
                        Err(_) => continue,
                    };
                    self.upsert_unit_states(&unit_name, &unit_props, &mut unit_states)
                        .expect("Failed to upsert '{}' into map of unit state machines.");
                }
            }
        }

        // Infinitely process Unit{Removed,New} signals.
        loop {
            for msg in self.connection.incoming(self.loop_timeout) {
                if let Some(msg_body) = UnitNew::from_message(&msg) {
                    self.handle_unit_new(&msg_body, &mut unit_states)?;
                } else if let Some(msg_body) = UnitRemoved::from_message(&msg) {
                    self.handle_unit_removed(&msg_body, &mut unit_states);
                } else if let Some(msg_body) = PropertiesChanged::from_message(&msg) {
                    self.handle_properties_changed(&msg, &msg_body, &mut unit_states)?;
                } else {
                    eprintln!("Unexpected message received: {:?}", msg);
                };
            }
            if self.loop_once {
                return Ok(());
            }
        }
    }

    // Call `org.freedesktop.DBus.Properties.GetAll`.
    //
    // This interface and method is widely implemented. Call it on bus name
    // `org.freedesktop.systemd1`, path name `unit_path`. The method accepts an `interface_name`,
    // which defines which interface is being queried.
    //
    // Return the response, or an error if one occurs. An error will be returned if the unit
    // corresponding to `unit_path` has been unloaded.
    fn call_properties_get_all(
        &self,
        unit_path: &Path,
    ) -> Result<HashMap<String, Variant<Box<dyn RefArg + 'static>>>, CrateDBusError> {
        self.get_conn_path(unit_path)
            .get_all("org.freedesktop.systemd1.Unit")
            .map_err(|err: DBusError| {
                CrateDBusError::CallOrgFreedesktopDBusPropertiesGetAll(format!("{}", err))
            })
    }

    // Call `org.freedesktop.systemd1.Manager.GetUnit`.
    //
    // Return the systemd unit path for `unit_name`, or an error if the unit is not loaded.
    fn call_manager_get_unit(&self, unit_name: &str) -> Result<Path, CrateDBusError> {
        self.get_conn_path(&wrap_path_for_systemd())
            .get_unit(unit_name)
            .map_err(|err: DBusError| {
                CrateDBusError::CallOrgFreedesktopSystemd1ManagerGetUnit(format!("{}", err))
            })
    }

    // Call `org.freedesktop.systemd1.Manager.Subscribe`.
    //
    // By default, the manager will *not* emit most signals. Enable them.
    fn call_manager_subscribe(&self) -> Result<(), CrateDBusError> {
        self.get_conn_path(&wrap_path_for_systemd())
            .subscribe()
            .map_err(|err: DBusError| {
                CrateDBusError::CallOrgFreedesktopSystemd1ManagerSubscribe(format!("{}", err))
            })
    }

    // Delete the given unit's state from `unit_states`, if present.
    fn forget_unit_state(unit_name: &str, unit_states: &mut HashMap<String, UnitStateMachine>) {
        unit_states.remove(unit_name);
    }

    // Generate callback for use in case a unit state machine changes.
    fn gen_on_change<'a>(
        &'a self,
        unit_name: &'a str,
    ) -> impl Fn(&UnitStateMachine, Option<ActiveState>) + 'a {
        move |usm: &UnitStateMachine, old_state: Option<ActiveState>| {
            let active_state = usm.active_state();
            let matching_rules: Vec<&Rule> = self.settings.rules.iter().collect();
            let matching_rules = get_rules_matching_name(&matching_rules, &unit_name);
            let matching_rules = get_rules_matching_active_state(&matching_rules, active_state);

            for matching_rule in &matching_rules {
                for notifier_name in &matching_rule.notifiers {
                    let notifier =
                        self.settings.notifiers.get(notifier_name).expect(
                            &format!("Failed to get notifier named '{}'", notifier_name)[..],
                        );

                    let header_bus_name = notifier.get_bus_name();
                    let header_path = make_path_like_bus_name(&header_bus_name);
                    let header_interface = wrap_interface_for_killjoy_notifier();
                    let header_member = wrap_member_for_notify();

                    let body_timestamp = usm.timestamp();
                    let body_unit_name = &unit_name;
                    // order from newest to oldest
                    let mut body_active_states: Vec<String> = vec![String::from(active_state)];
                    if let Some(old_state) = old_state {
                        body_active_states.push(String::from(old_state));
                    }

                    let msg = Message::method_call(
                        &header_bus_name,
                        &header_path,
                        &header_interface,
                        &header_member,
                    )
                    .append3::<u64, &str, &Vec<String>>(
                        body_timestamp,
                        body_unit_name,
                        &body_active_states,
                    );

                    let conn = Connection::get_private(notifier.bus_type).expect(
                        &format!("Failed to connect to {:?} D-Bus bus.", notifier.bus_type)[..],
                    );
                    if let Err(err) = conn.send_with_reply_and_block(msg, 5000) {
                        eprintln!(
                            "Error occurred when contacting notifier \"{}\": {}",
                            notifier_name, err
                        );
                    }
                }
            }
        }
    }

    // Get a `ConnPath` for `org.freedesktop.systemd1` and the given object path.
    fn get_conn_path<'a: 'b, 'b>(&'a self, path: &'b Path) -> ConnPath<'b, &Connection> {
        let conn = &self.connection;
        let bus_name = wrap_bus_name_for_systemd();
        let path = path.to_owned();
        let timeout = 1000; // milliseconds
        ConnPath {
            conn,
            dest: bus_name,
            path,
            timeout,
        }
    }

    // Call `org.freedesktop.systemd1.Manager.ListUnits`.
    //
    // This method "returns an array with all currently loaded units."
    fn call_manager_list_units(&self) -> Result<Vec<String>, CrateDBusError> {
        self.get_conn_path(&wrap_path_for_systemd())
            .list_units()
            .map(|units| units.into_iter().map(|unit| unit.0).collect())
            .map_err(|err| {
                CrateDBusError::CallOrgFreedesktopSystemd1ManagerListUnits(format!("{}", err))
            })
    }

    // Handle the UnitNew signal.
    //
    // If any calls to systemd fail, assume the unit has been unloaded, and return Ok. If any calls
    // to D-Bus fail, assume something worse has happened, and return Err.
    fn handle_unit_new(
        &self,
        msg_body: &UnitNew,
        unit_states: &mut HashMap<String, UnitStateMachine>,
    ) -> Result<(), CrateDBusError> {
        let borrowed_rules: Vec<&Rule> = self.settings.rules.iter().collect();
        let unit_name: &String = &msg_body.arg0;
        let unit_path: &Path = &msg_body.arg1;
        if rules_match_name(&borrowed_rules, unit_name) {
            self.subscribe_properties_changed(&unit_path)?;
            let unit_props = match self.call_properties_get_all(&unit_path) {
                Ok(unit_props) => unit_props,
                Err(_) => return Ok(()),
            };
            self.upsert_unit_states(unit_name, &unit_props, unit_states)
                .expect("Failed to upsert '{}' into map of unit state machines.");
        }
        Ok(())
    }

    // Handle the UnitRemoved signal.
    fn handle_unit_removed(
        &self,
        msg_body: &UnitRemoved,
        unit_states: &mut HashMap<String, UnitStateMachine>,
    ) {
        let borrowed_rules: Vec<&Rule> = self.settings.rules.iter().collect();
        let unit_name: &String = &msg_body.arg0;
        let unit_path: &Path = &msg_body.arg1;
        if rules_match_name(&borrowed_rules, unit_name) {
            if let Err(err) = self.unsubscribe_properties_changed(&unit_path) {
                panic!("Failed to handle UnitRemoved signal: {}", err);
            }
            Self::forget_unit_state(unit_name, unit_states);
        }
    }

    // Handle the PropertiesChanged signal.
    //
    // The message headers tell us where the signal is coming from:
    //
    // * msg.sender should be org.freedesktop.systemd1, as that's the only sender we've subscribed
    //   to.
    // * msg.path varies. It's a value like /org/freedesktop/systemd1/unit/syncthing_2eservice.
    // * msg.interface is org.freedesktop.dbus.Properties.
    // * msg.member is PropertiesChanged.
    //
    // Finally, msg_body.interface tells us which other interface on the same sender + path has
    // changed. It's a value like org.freedesktop.systemd1.Unit or org.freedesktop.systemd1.Service.
    fn handle_properties_changed(
        &self,
        msg: &Message,
        msg_body: &PropertiesChanged,
        unit_states: &mut HashMap<String, UnitStateMachine>,
    ) -> Result<(), CrateDBusError> {
        // We only care about the properties exposed by this interface.
        if msg_body.interface != INTERFACE_FOR_SYSTEMD_UNIT {
            return Ok(());
        }

        // Get path of unit that changed.
        let unit_path: Path = msg.path().ok_or_else(|| CrateDBusError::MessageLacksPath)?;

        // Translate the signal's path into a unit name.
        //
        // One can ask systemd for the properties of a fictitious unit, e.g.
        // /org/freedesktop/systemd1/unit/dbusss_2eservice, and it will respond. Thus, we can rely
        // on systemd to respond here.
        let unit_name: String = self
            .get_conn_path(&unit_path)
            .get(INTERFACE_FOR_SYSTEMD_UNIT, "Id")
            .map_err(|err| CrateDBusError::GetOrgFreedesktopSystemd1UnitId(format!("{}", err)))?
            .0
            .as_str()
            .ok_or_else(|| CrateDBusError::CastOrgFreedesktopSystemd1UnitId)?
            .to_string();

        // If the ActiveState property is missing, assume it didn't change.
        match self.upsert_unit_states(&unit_name[..], &msg_body.changed_properties, unit_states) {
            Ok(_) => Ok(()),
            Err(err) => match err {
                CrateDBusError::PropertiesLacksActiveState => Ok(()),
                _ => Err(err),
            },
        }
    }

    // Upsert the state machines in `unit_states` as appropriate.
    fn upsert_unit_states(
        &self,
        unit_name: &str,
        unit_props: &UnitProps,
        unit_states: &mut HashMap<String, UnitStateMachine>,
    ) -> Result<(), CrateDBusError> {
        // Get unit's current ActiveState, and time at which it entered that state.
        let active_state: ActiveState = get_active_state(&unit_props)?;
        let timestamp: u64 = get_monotonic_timestamp(active_state, unit_props)?;

        // Upsert unit state machine.
        let on_change = self.gen_on_change(&unit_name);
        unit_states
            .entry(unit_name.to_string())
            .and_modify(|usm| usm.update(active_state, timestamp, &on_change))
            .or_insert_with(|| UnitStateMachine::new(active_state, timestamp, &on_change));
        Ok(())
    }

    // Subscribe to the `org.freedesktop.systemd1.Manager.UnitNew` signal.
    fn subscribe_manager_unit_new(&self) -> Result<(), CrateDBusError> {
        let bus_name = wrap_bus_name_for_systemd();
        let path = wrap_path_for_systemd();
        let match_str: String = UnitNew::match_str(Some(&bus_name), Some(&path));
        self.connection
            .add_match(&match_str)
            .map_err(|err: DBusError| CrateDBusError::AddMatch(match_str, format!("{}", err)))
    }

    // Subscribe to the `org.freedesktop.systemd1.Manager.UnitRemoved` signal.
    fn subscribe_manager_unit_removed(&self) -> Result<(), CrateDBusError> {
        let bus_name = wrap_bus_name_for_systemd();
        let path = wrap_path_for_systemd();
        let match_str: String = UnitRemoved::match_str(Some(&bus_name), Some(&path));
        self.connection
            .add_match(&UnitRemoved::match_str(Some(&bus_name), Some(&path)))
            .map_err(|err: DBusError| CrateDBusError::AddMatch(match_str, format!("{}", err)))
    }

    // Subscribe to the `org.freedesktop.DBus.Properties.PropertiesChanged` signal.
    fn subscribe_properties_changed(&self, unit_path: &Path) -> Result<(), CrateDBusError> {
        let bus_name = wrap_bus_name_for_systemd();
        let match_str: String = PropertiesChanged::match_str(Some(&bus_name), Some(&unit_path));
        self.connection
            .add_match(&match_str)
            .map_err(|err: DBusError| CrateDBusError::AddMatch(match_str, format!("{}", err)))
    }

    // Unsubscribe from the `org.freedesktop.DBus.Properties.PropertiesChanged` signal.
    fn unsubscribe_properties_changed(&self, unit_path: &Path) -> Result<(), CrateDBusError> {
        let bus_name = wrap_bus_name_for_systemd();
        let match_str: String = PropertiesChanged::match_str(Some(&bus_name), Some(&unit_path));
        self.connection
            .remove_match(&match_str)
            .map(|_| ())
            .map_err(|err: DBusError| CrateDBusError::RemoveMatch(match_str, format!("{}", err)))
    }
}

// Tell which rules match the given unit name.
fn get_rules_matching_name<'a>(rules: &[&'a Rule], unit_name: &str) -> Vec<&'a Rule> {
    rules
        .iter()
        .cloned() // &&Rule → &Rule
        .filter(|rule: &&Rule| rule.expression.matches(unit_name))
        .collect()
}

// Tell which rules match the given unit state.
fn get_rules_matching_active_state<'a>(rules: &[&'a Rule], target: ActiveState) -> Vec<&'a Rule> {
    rules
        .iter()
        .cloned() // &&Rule → &Rule
        .filter(|rule: &&Rule| {
            rule.active_states
                .iter()
                .any(|active_state| *active_state == target)
        })
        .collect()
}

// Return the timestamp indicating when the given state was most recently entered.
fn get_monotonic_timestamp(
    active_state: ActiveState,
    unit_props: &UnitProps,
) -> Result<u64, CrateDBusError> {
    let timestamp_key: &'static str = get_monotonic_timestamp_key(active_state);
    unit_props
        .get(timestamp_key)
        .ok_or_else(|| CrateDBusError::PropertiesLacksTimestamp(active_state, timestamp_key))?
        .0
        .as_u64()
        .ok_or_else(|| CrateDBusError::CastOrgFreedesktopSystemd1UnitTimestamp(timestamp_key))
}

// Return name of the monotonic timestamp indicating when the given state was most recently entered.
fn get_monotonic_timestamp_key(active_state: ActiveState) -> &'static str {
    match active_state {
        ActiveState::Activating => "InactiveExitTimestampMonotonic",
        ActiveState::Active => "ActiveEnterTimestampMonotonic",
        ActiveState::Deactivating => "ActiveExitTimestampMonotonic",
        ActiveState::Failed => "InactiveEnterTimestampMonotonic",
        ActiveState::Inactive => "InactiveEnterTimestampMonotonic",
    }
}

// Return the value of the ActiveState property.
fn get_active_state(unit_props: &UnitProps) -> Result<ActiveState, CrateDBusError> {
    let active_state_str: &str = unit_props
        .get("ActiveState")
        .ok_or_else(|| CrateDBusError::PropertiesLacksActiveState)?
        .0
        .as_str()
        .ok_or_else(|| CrateDBusError::CastOrgFreedesktopSystemd1UnitActiveState)?;
    ActiveState::try_from(active_state_str)
        .map_err(CrateDBusError::DecodeOrgFreedesktopSystemd1UnitActiveState)
}

// Given a bus name foo.bar.Biz1, make path /foo/bar/Biz1.
//
// Will panic if unable to make a string from the contents of `bus_name`, or if the Path object
// being created does not contain a valid path name.
fn make_path_like_bus_name(bus_name: &BusName) -> Path<'static> {
    let mut path_str = bus_name
        .as_cstr()
        .to_str()
        .expect("Failed to create string from BusName.")
        .replace(".", "/");
    path_str.insert(0, '/');
    Path::new(path_str)
        .expect(&format!("Failed to convert bus name to path. Bus name: {}", bus_name)[..])
        .to_owned()
}

// Tell whether at least one rule matches the given unit name.
fn rules_match_name(rules: &[&Rule], unit_name: &str) -> bool {
    !get_rules_matching_name(rules, unit_name).is_empty()
}

// Wrap BUS_NAME_FOR_SYSTEMD.
fn wrap_bus_name_for_systemd() -> BusName<'static> {
    BusName::new(BUS_NAME_FOR_SYSTEMD)
        .expect(&format!("Failed to create BusName from '{}'", BUS_NAME_FOR_SYSTEMD)[..])
}

// Wrap PATH_FOR_SYSTEMD.
fn wrap_path_for_systemd() -> Path<'static> {
    Path::new(PATH_FOR_SYSTEMD)
        .expect(&format!("Failed to create Path from '{}'", PATH_FOR_SYSTEMD)[..])
}

fn wrap_interface_for_killjoy_notifier() -> Interface<'static> {
    let interface_str = "name.jerebear.KilljoyNotifier1";
    Interface::new(interface_str)
        .expect(&format!("Failed to create Interface from '{}'", interface_str)[..])
}

fn wrap_member_for_notify() -> Member<'static> {
    let member_str = "Notify";
    Member::new(member_str).expect(&format!("Failed to create Member from '{}'", member_str)[..])
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::settings::{test_utils, Expression};

    // get_monotonic_timestamp_key()
    #[test]
    fn test_get_monotonic_timestamp_key() {
        for act_st in vec![
            ActiveState::Activating,
            ActiveState::Active,
            ActiveState::Deactivating,
            ActiveState::Failed,
            ActiveState::Inactive,
        ] {
            assert!(get_monotonic_timestamp_key(act_st).contains("Monotonic"));
        }
    }

    #[test]
    fn test_make_path_like_bus_name() {
        let bus_name = BusName::new(BUS_NAME_FOR_SYSTEMD)
            .expect(&format!("Failed to create BusName from {}", BUS_NAME_FOR_SYSTEMD)[..]);
        let path = make_path_like_bus_name(&bus_name);
        let path_str = path
            .as_cstr()
            .to_str()
            .expect(&format!("Failed to create string from {}", path));
        assert_eq!(path_str, "/org/freedesktop/systemd1");
    }

    // Let the unit name match zero of two rules.
    #[test]
    fn test_match_rules_and_names_v1() {
        let mut rules = vec![test_utils::gen_system_rule(), test_utils::gen_system_rule()];
        rules[0].expression = Expression::UnitName("foo.mount".to_owned());
        rules[1].expression = Expression::UnitType(".mount".to_owned());
        let borrowed_rules: Vec<&Rule> = rules.iter().collect();

        let unit_name = "bar.service";

        let matching_rules = get_rules_matching_name(&borrowed_rules, unit_name);
        assert_eq!(matching_rules.len(), 0);

        assert!(!rules_match_name(&borrowed_rules, unit_name));
    }

    // Let the unit name match one of two rules.
    #[test]
    fn test_match_rules_and_names_v2() {
        let mut rules = vec![test_utils::gen_system_rule(), test_utils::gen_system_rule()];
        rules[0].expression = Expression::UnitName("foo.mount".to_owned());
        rules[1].expression = Expression::UnitType(".mount".to_owned());
        let borrowed_rules: Vec<&Rule> = rules.iter().collect();

        let unit_name = "bar.mount";

        let matching_rules = get_rules_matching_name(&borrowed_rules, unit_name);
        assert_eq!(matching_rules.len(), 1);

        assert!(rules_match_name(&borrowed_rules, unit_name));
    }

    // Let the unit name match two of two rules.
    #[test]
    fn test_match_rules_and_names_v3() {
        let mut rules = vec![test_utils::gen_system_rule(), test_utils::gen_system_rule()];
        rules[0].expression = Expression::UnitName("foo.mount".to_owned());
        rules[1].expression = Expression::UnitType(".mount".to_owned());
        let borrowed_rules: Vec<&Rule> = rules.iter().collect();

        let unit_name = "foo.mount";

        let matching_rules = get_rules_matching_name(&borrowed_rules, unit_name);
        assert_eq!(matching_rules.len(), 2);

        assert!(rules_match_name(&borrowed_rules, unit_name));
    }

    // Let the unit ActiveState match zero of two rules.
    #[test]
    fn test_match_rules_and_active_state_v1() {
        let mut rules = vec![test_utils::gen_system_rule(), test_utils::gen_system_rule()];
        rules[0].active_states.insert(ActiveState::Activating);
        rules[0].active_states.insert(ActiveState::Active);
        rules[1].active_states.insert(ActiveState::Active);
        let borrowed_rules: Vec<&Rule> = rules.iter().collect();

        let active_state = ActiveState::Inactive;

        let matching_rules = get_rules_matching_active_state(&borrowed_rules, active_state);
        assert_eq!(matching_rules.len(), 0);
    }

    // Let the unit ActiveState match one of two rules.
    #[test]
    fn test_match_rules_and_active_state_v2() {
        let mut rules = vec![test_utils::gen_system_rule(), test_utils::gen_system_rule()];
        rules[0].active_states.insert(ActiveState::Activating);
        rules[0].active_states.insert(ActiveState::Active);
        rules[1].active_states.insert(ActiveState::Active);
        let borrowed_rules: Vec<&Rule> = rules.iter().collect();

        let active_state = ActiveState::Activating;

        let matching_rules = get_rules_matching_active_state(&borrowed_rules, active_state);
        assert_eq!(matching_rules.len(), 1);
    }

    // Let the unit ActiveState match two of two rules.
    #[test]
    fn test_match_rules_and_active_state_v3() {
        let mut rules = vec![test_utils::gen_system_rule(), test_utils::gen_system_rule()];
        rules[0].active_states.insert(ActiveState::Activating);
        rules[0].active_states.insert(ActiveState::Active);
        rules[1].active_states.insert(ActiveState::Active);
        let borrowed_rules: Vec<&Rule> = rules.iter().collect();
        let active_state = ActiveState::Active;

        let matching_rules = get_rules_matching_active_state(&borrowed_rules, active_state);
        assert_eq!(matching_rules.len(), 2);
    }

    #[test]
    fn test_wrap_bus_name_for_systemd() {
        wrap_bus_name_for_systemd();
    }

    #[test]
    fn test_wrap_path_for_systemd() {
        wrap_path_for_systemd();
    }

    #[test]
    fn test_wrap_interface_for_killjoy_notifier() {
        wrap_interface_for_killjoy_notifier();
    }

    #[test]
    fn test_wrap_member_for_notify() {
        wrap_member_for_notify();
    }

    #[test]
    fn test_interface_for_systemd_unit() {
        Interface::new(INTERFACE_FOR_SYSTEMD_UNIT).expect(
            &format!(
                "Failed to create Interface from {}",
                INTERFACE_FOR_SYSTEMD_UNIT
            )[..],
        );
    }
}
