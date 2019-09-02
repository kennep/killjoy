killjoy
=======

Monitor systemd units.

killjoy is a systemd unit monitoring application. It discovers systemd units and
tracks their states. When a unit changes to a state of interest, killjoy
contacts notifiers. Examples of notifiers include:

* [killjoy Notifier: Logfile](https://github.com/Ichimonji10/killjoy-notifier-logfile)
* [killjoy Notifier: Notification](https://github.com/Ichimonji10/killjoy-notifier-notification)

Documentation is available on the web at [docs.rs](https://docs.rs/killjoy). It
can also be locally generated with `cargo doc --open`.

License
-------

killjoy is licensed under the GPLv3 or any later version.
