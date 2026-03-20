#!/usr/bin/env bash
set -euo pipefail

# Build optimized binary and then produce an RPM package.
cargo build --release
cargo generate-rpm

echo "Package created in target/generate-rpm/"
