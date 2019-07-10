#!/usr/bin/env bash
# coding=utf-8
#
# Regenerate certain files in src/generated/.
set -euo pipefail

~/.cargo/bin/dbus-codegen-rust \
    --destination org.freedesktop.systemd1 \
    --path /org/freedesktop/systemd1 \
    > src/generated/org_freedesktop_systemd1.rs
