#!/usr/bin/env bash
# coding=utf-8
set -euo pipefail

find scripts -type f -name '*.sh' -exec shellcheck '{}' ';'
cargo check
cargo test
cargo clippy -- --deny warnings  # treat warnings as errors
cargo fmt && git diff --exit-code
