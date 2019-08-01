killjoy
=======

**Killjoy is in the early stages of development, and many features are absent or
poorly tested. Read the following description with skepticism until this warning
is removed.**

Killjoy is a systemd unit monitoring application.

What is systemd?

> systemd is a suite of basic building blocks for a Linux system. It provides a
> system and service manager that runs as PID 1 and starts the rest of the
> system.

Units are the resources that systemd knows how to manage. For example, the unit
corresponding to the nginx web server might be `nginx.service`, and the unit
corresponding to the `/boot` mount point might be `boot.mount`, though naming
can vary per Linux distribution.

Killjoy watches for a configurable list of events, such as "`nginx.service`
failed," or "`my-backup.service` is activating, active, or deactivating."
Killjoy responds to these events by reaching out across a D-Bus and contacting a
configurable list of notifiers. In turn, the notifiers are responsible for
generating desktop pop-ups, sending emails, or otherwise taking action.

A small number of notifiers are developed alongside killjoy. However, the clear
separation between the watcher (killjoy) and the notifiers means that anyone can
write and distribute a custom notifier at any time, with no changes to killjoy
itself. Want to start the WiFi coffee maker when the daily backup service kicks
off? Go for it.

Killjoy is inspired by
[sagbescheid](https://sagbescheid.readthedocs.io/en/latest/),
[SystemdMon](https://github.com/joonty/systemd_mon),
[pynagsystemd](https://github.com/kbytesys/pynagsystemd), and
[`OnFailure=`](https://www.freedesktop.org/software/systemd/man/systemd.unit.html),
but there are differences in efficiency, reliability, features, and flexibility.

Killjoy assumes knowledge of systemd. For more information about systemd, see
its [project page](https://freedesktop.org/wiki/Software/systemd/) and
[systemd(1)](https://www.freedesktop.org/software/systemd/man/systemd.html),
especially the section on
[concepts](https://www.freedesktop.org/software/systemd/man/systemd.html#Concepts).

Dependencies
------------

Most dependencies used by Killjoy are pure Rust libraries and are listed in
`Cargo.toml`. However, Killjoy indirectly requires libdbus at runtime. (On
Ubuntu, install `libdbus-1-dev`.) For details, see the Rust dbus library's
[requirements](https://github.com/diwic/dbus-rs#requirements).

License
-------

Killjoy is licensed under the GPLv3 or any later version.
