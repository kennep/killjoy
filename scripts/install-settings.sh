#!/usr/bin/env bash
# coding=utf-8
#
# Install a settings file.
set -euo pipefail

install -Dm644 scripts/settings.json ~/.config/killjoy/settings.json
