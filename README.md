killjoy
=======

**Killjoy is in the early stages of development, and many features are absent or
poorly tested. Read the following description with skepticism until this warning
is removed.**

Killjoy is a systemd unit monitoring application.

Killjoy watches for a configurable list of events, such as "a mount point
failed" or "my backup service is active," and responds by contacting a
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
epsecially the section on
[concepts](https://www.freedesktop.org/software/systemd/man/systemd.html#Concepts).

License
-------

Killjoy is licensed under the GPLv3 or any later version.
