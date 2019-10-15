#!/usr/bin/env bash
# coding=utf-8
#
# Regenerate certain files in src/generated/.
set -euo pipefail

~/.cargo/bin/dbus-codegen-rust \
    --destination org.freedesktop.systemd1 \
    --path /org/freedesktop/systemd1 \
    > src/generated/org_freedesktop_systemd1.rs
sed -i 's/<arg::RefArg/<dyn arg::RefArg/g' \
    src/generated/org_freedesktop_systemd1.rs
cargo fmt
