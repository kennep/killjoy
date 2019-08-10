// Logic for interacting with D-Bus buses.

use std::collections::HashMap;
use std::convert::TryFrom;

use dbus::arg::Variant;
use dbus::{
    BusName, BusType, ConnPath, Connection, Error as DBusError, Interface, Member, Message, Path,
    SignalArgs,
};

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
    pub fn run(&self) -> Result<(), i32> {
        if let Err(err) = self.call_manager_subscribe() {
            eprintln!(
                "Monitoring thread for bus {} failed to subscribe to systemd signals. Exiting. Underlying error: {}",
                self.connection.unique_name(), err
            );
            return Err(1);
        }

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
        if let Err(err) = self.subscribe_manager_unit_removed() {
            eprintln!(
                "Monitoring thread for bus {} failed to subscribe to the UnitRemoved signal. Exiting. Underlying error: {}",
                self.connection.unique_name(), err
            );
            return Err(1);
        }
        if let Err(err) = self.subscribe_manager_unit_new() {
            eprintln!(
                "Monitoring thread for bus {} failed to subscribe to the UnitNew signal. Exiting. Underlying error: {}",
                self.connection.unique_name(), err
            );
            return Err(1);
        }

        // Review extant units, and act on interesting ones.
        let mut unit_states: HashMap<String, UnitStateMachine> = HashMap::new();
        let unit_names = match self.call_manager_list_units() {
            Ok(unit_names) => unit_names,
            Err(err) => {
                eprintln!(
                    "Monitoring thread for bus {} failed to get list of unit names. Exiting. Underlying error: {}",
                    self.connection.unique_name(), err
                );
                return Err(1);
            }
        };
        unit_names
            .iter()
            .filter(|unit_name: &&String| {
                let borrowed_rules: Vec<&Rule> = self.settings.rules.iter().collect();
                rules_match_name(&borrowed_rules, unit_name)
            })
            .for_each(|unit_name: &String| {
                self.subscribe_properties_changed_or_suppress(unit_name);
                self.learn_unit_state_or_suppress(unit_name, &mut unit_states);
            });

        // Infinitely process Unit{Removed,New} signals.
        loop {
            for msg in self.connection.incoming(self.loop_timeout) {
                if let Some(msg_body) = UnitNew::from_message(&msg) {
                    self.handle_unit_new(&msg_body, &mut unit_states);
                } else if let Some(msg_body) = UnitRemoved::from_message(&msg) {
                    self.handle_unit_removed(&msg_body, &mut unit_states);
                } else if let Some(msg_body) = PropertiesChanged::from_message(&msg) {
                    self.handle_properties_changed(&msg, &msg_body, &mut unit_states);
                } else {
                    eprintln!("Unexpected message received: {:?}", msg);
                };
            }
            if self.loop_once {
                return Ok(());
            }
        }
    }

    // Call `org.freedesktop.systemd1.Manager.Subscribe`.
    //
    // By default, the manager will *not* emit most signals. Enable them.
    fn call_manager_subscribe(&self) -> Result<(), DBusError> {
        self.get_conn_path(&wrap_path_for_systemd()).subscribe()
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
                    let notifier = self.settings.notifiers.get(notifier_name).unwrap();

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
                    match conn.send_with_reply_and_block(msg, 5000) {
                        Ok(_) => {}
                        Err(err) => {
                            eprintln!(
                                "Error occurred when contacting notifier \"{}\": {}",
                                notifier_name, err
                            );
                        }
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
    fn call_manager_list_units(&self) -> Result<Vec<String>, DBusError> {
        Ok(self
            .get_conn_path(&wrap_path_for_systemd())
            .list_units()?
            .into_iter()
            .map(|unit| unit.0)
            .collect())
    }

    // Handle the UnitNew signal.
    fn handle_unit_new(
        &self,
        msg_body: &UnitNew,
        unit_states: &mut HashMap<String, UnitStateMachine>,
    ) {
        let borrowed_rules: Vec<&Rule> = self.settings.rules.iter().collect();
        let unit_name = &msg_body.arg0;
        if rules_match_name(&borrowed_rules, unit_name) {
            self.subscribe_properties_changed_or_suppress(unit_name);
            self.learn_unit_state_or_suppress(unit_name, unit_states);
        }
    }

    // Handle the UnitRemoved signal.
    fn handle_unit_removed(
        &self,
        msg_body: &UnitRemoved,
        unit_states: &mut HashMap<String, UnitStateMachine>,
    ) {
        let unit_name = &msg_body.arg0;
        self.unsubscribe_properties_changed_or_suppress(unit_name);
        Self::forget_unit_state(unit_name, unit_states);
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
    ) {
        // The properties we're interested in are exposed on org.freedesktop.systemd1.Unit.
        if msg_body.interface != INTERFACE_FOR_SYSTEMD_UNIT {
            return;
        }

        // Get the unit's ActiveState property. If ActiveState isn't in changed_properties, then no
        // work needs to be done.
        let active_state: ActiveState = match msg_body.changed_properties.get("ActiveState") {
            Some(active_state_variant) => {
                // active_state_variant: dbus::arg::Variant<Box<dbus::arg::RefArg + 'static>>
                let active_state_str = active_state_variant.0.as_str().unwrap();
                ActiveState::try_from(active_state_str).unwrap()
            }
            None => return,
        };

        // Get the timestamp at which that state was last entered. If ActiveState changed, then we
        // assume that the corresponding timestamp must be present.
        let msg_path: Path = msg.path().expect("Failed to get path from signal headers.");
        let timestamp_key = get_timestamp_key(active_state);
        let timestamp: u64 = match msg_body.changed_properties.get(timestamp_key) {
            Some(timestamp_variant) => timestamp_variant.0.as_u64().unwrap(),
            None => panic!(format!(
                "A PropertiesChanged signal was received, indicating that {:?} changed state to {:?}. However, the signal didn't include a corresponding timestamp named {}.",
                msg_path, active_state, timestamp_key
            )),
        };

        // Translate the signal's path into a unit name.
        let unit_id_result = self
            .get_conn_path(&msg_path)
            .get(INTERFACE_FOR_SYSTEMD_UNIT, "Id");
        let unit_name: String = match unit_id_result {
            Ok(unit_id_variant) => unit_id_variant.0.as_str().unwrap().to_string(),
            Err(err) => {
                eprintln!(
                    "Failed to get unit name for {:?}. State change may have been missed. Error: {}",
                    msg_path, err
                );
                return;
            }
        };

        // Update unit state machine.
        let on_change = self.gen_on_change(&unit_name);
        unit_states
            .entry(unit_name.clone())
            .and_modify(|usm| usm.update(active_state, timestamp, &on_change))
            .or_insert_with(|| UnitStateMachine::new(active_state, timestamp, &on_change));
    }

    // Get the given unit's state, and update `unit_states` as appropriate.
    //
    // This method asks systemd for several pieces of information about `unit_name`, such as its
    // state. If systemd returns an error in response to those queries, this method immediately
    // returns them, without making any changes to self. If you're sure that `unit_path` is correct
    // (i.e. not mangled by a typo), it is probable that systemd unloaded the unit from memory. In
    // this case, the error may be ignored, as `unfollow_unit` will catch the `UnitRemoved` signal
    // and pop an entry from `unit_states`.
    //
    // If an unknown unit state is encountered, this method will panic. The `ActiveState` struct
    // lists all known states.
    fn learn_unit_state(
        &self,
        unit_name: &str,
        unit_states: &mut HashMap<String, UnitStateMachine>,
    ) -> Result<(), DBusError> {
        // Get unit properties.
        let path: Path = self
            .get_conn_path(&wrap_path_for_systemd())
            .get_unit(unit_name)?;
        let unit_props: HashMap<String, Variant<_>> = self
            .get_conn_path(&path)
            .get_all("org.freedesktop.systemd1.Unit")?;

        // Get and decode unit's ActiveState property.
        let active_state_str: &str = unit_props.get("ActiveState").unwrap().0.as_str().unwrap();
        let active_state = ActiveState::try_from(active_state_str).unwrap();

        // Get timestamp at which that state was last entered.
        let timestamp_key = get_timestamp_key(active_state);
        let timestamp = unit_props.get(timestamp_key).unwrap().0.as_u64().unwrap();

        // Update unit state machine.
        let on_change = self.gen_on_change(&unit_name);
        unit_states
            .entry(unit_name.to_string())
            .and_modify(|usm| usm.update(active_state, timestamp, &on_change))
            .or_insert_with(|| UnitStateMachine::new(active_state, timestamp, &on_change));

        Ok(())
    }

    fn learn_unit_state_or_suppress(
        &self,
        unit_name: &str,
        unit_states: &mut HashMap<String, UnitStateMachine>,
    ) {
        if let Err(err) = self.learn_unit_state(&unit_name, unit_states) {
            eprintln!("Failed to learn ActiveState for {}: {}", unit_name, err)
        }
    }

    // Subscribe to the `org.freedesktop.systemd1.Manager.UnitNew` signal.
    fn subscribe_manager_unit_new(&self) -> Result<(), DBusError> {
        let bus_name = wrap_bus_name_for_systemd();
        let path = wrap_path_for_systemd();
        self.connection
            .add_match(&UnitNew::match_str(Some(&bus_name), Some(&path)))
    }

    // Subscribe to the `org.freedesktop.systemd1.Manager.UnitRemoved` signal.
    fn subscribe_manager_unit_removed(&self) -> Result<(), DBusError> {
        let bus_name = wrap_bus_name_for_systemd();
        let path = wrap_path_for_systemd();
        self.connection
            .add_match(&UnitRemoved::match_str(Some(&bus_name), Some(&path)))
    }

    // Subscribe to the `PropertiesChanged` signal for the given unit.
    fn subscribe_properties_changed(&self, unit_name: &str) -> Result<(), DBusError> {
        let bus_name = wrap_bus_name_for_systemd();
        let path = self
            .get_conn_path(&wrap_path_for_systemd())
            .get_unit(unit_name)?;
        let match_str = &PropertiesChanged::match_str(Some(&bus_name), Some(&path));
        self.connection.add_match(&match_str)
    }

    fn subscribe_properties_changed_or_suppress(&self, unit_name: &str) {
        if let Err(err) = self.subscribe_properties_changed(&unit_name) {
            eprintln!(
                "Failed to subscribe to PropertiesChanged for {}: {}",
                unit_name, err
            );
        }
    }

    fn unsubscribe_properties_changed(&self, unit_name: &str) -> Result<(), DBusError> {
        let bus_name = wrap_bus_name_for_systemd();
        let path = self
            .get_conn_path(&wrap_path_for_systemd())
            .get_unit(unit_name)?;
        let match_str = &PropertiesChanged::match_str(Some(&bus_name), Some(&path));
        self.connection.remove_match(&match_str)
    }

    fn unsubscribe_properties_changed_or_suppress(&self, unit_name: &str) {
        if let Err(err) = self.unsubscribe_properties_changed(&unit_name) {
            eprintln!(
                "Failed to unsubscribe from PropertiesChanged for {}: {}",
                unit_name, err
            );
        }
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

// Return the D-Bus property indicating when the given state was most recently entered.
fn get_timestamp_key(active_state: ActiveState) -> &'static str {
    match active_state {
        ActiveState::Activating => "InactiveExitTimestampMonotonic",
        ActiveState::Active => "ActiveEnterTimestampMonotonic",
        ActiveState::Deactivating => "ActiveExitTimestampMonotonic",
        ActiveState::Failed => "InactiveEnterTimestampMonotonic",
        ActiveState::Inactive => "InactiveEnterTimestampMonotonic",
    }
}

// Given a bus name foo.bar.Biz1, make path /foo/bar/Biz1.
//
// Will panic if unable to make a string from the contents of `bus_name`, or if the Path object
// being created does not contain a valid path name.
fn make_path_like_bus_name(bus_name: &BusName) -> Path<'static> {
    let mut path_str = bus_name
        .as_cstr()
        .to_str()
        .expect("Failed to create string from contents of BusName.")
        .replace(".", "/");
    path_str.insert(0, '/');
    Path::new(path_str).unwrap().to_owned()
}

// Tell whether at least one rule matches the given unit name.
fn rules_match_name(rules: &[&Rule], unit_name: &str) -> bool {
    !get_rules_matching_name(rules, unit_name).is_empty()
}

// Wrap BUS_NAME_FOR_SYSTEMD.
fn wrap_bus_name_for_systemd() -> BusName<'static> {
    BusName::new(BUS_NAME_FOR_SYSTEMD).unwrap()
}

// Wrap PATH_FOR_SYSTEMD.
fn wrap_path_for_systemd() -> Path<'static> {
    Path::new(PATH_FOR_SYSTEMD).unwrap()
}

fn wrap_interface_for_killjoy_notifier() -> Interface<'static> {
    Interface::new("name.jerebear.KilljoyNotifier1").unwrap()
}

fn wrap_member_for_notify() -> Member<'static> {
    Member::new("Notify").unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::settings::{test_utils, Expression};

    #[test]
    fn test_make_path_like_bus_name() {
        let bus_name = BusName::new("com.example.App1").unwrap();
        let path = make_path_like_bus_name(&bus_name);
        let path_str = path
            .as_cstr()
            .to_str()
            .expect("Failed to create string from contents of Path.");
        assert_eq!(path_str, "/com/example/App1");
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
        Interface::new(INTERFACE_FOR_SYSTEMD_UNIT).unwrap();
    }
}
