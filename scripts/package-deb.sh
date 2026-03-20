#!/usr/bin/env bash
set -euo pipefail

# Build optimized binary and then produce a Debian package.
cargo build --release
cargo deb --no-build

echo "Package created in target/debian/"
