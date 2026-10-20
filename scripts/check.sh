#!/usr/bin/env bash
# Run the same required Rust gates as the shared CI workflow.
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

if [[ ${1:-} == -h || ${1:-} == --help ]]; then
  echo 'Usage: scripts/check.sh [fmt|check|clippy|test|deny|msrv ...]'
  echo 'Default: fmt clippy test deny msrv. Missing tools fail the check.'
  exit 0
fi

gates=("$@")
if [[ ${#gates[@]} -eq 0 ]]; then
  gates=(fmt clippy test deny msrv)
fi
for gate in "${gates[@]}"; do
  case "$gate" in
    fmt|check|clippy|test|deny|msrv) ;;
    *) echo "Unknown gate: $gate" >&2; exit 2 ;;
  esac
done

for gate in "${gates[@]}"; do
  printf '\nChecking %s\n' "$gate"
  case "$gate" in
    fmt) cargo +stable fmt --all -- --check ;;
    check) cargo +stable check --locked ;;
    clippy) cargo +stable clippy --locked --all-targets -- -D warnings ;;
    test) cargo +stable test --locked ;;
    deny) cargo deny --locked --all-features check ;;
    msrv)
      version=$(python3 - <<'PY'
import re
import tomllib
from pathlib import Path

version = tomllib.loads(Path('Cargo.toml').read_text()).get('package', {}).get('rust-version')
if not isinstance(version, str) or not re.fullmatch(r'1\.(0|[1-9][0-9]*)(?:\.(0|[1-9][0-9]*))?', version):
    raise SystemExit('A root package with an explicit rust-version is required')
print(version + '.0' if version.count('.') == 1 else version)
PY
)
      cargo +"$version" check --locked --all-targets
      ;;
  esac
done
printf '\nPassed requested gates: %s\n' "${gates[*]}"
