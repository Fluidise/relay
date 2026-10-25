#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# Verify python3
if ! command -v python3 >/dev/null 2>&1; then
    echo "Error: python3 is required to run the benchmark harness." >&2
    exit 1
fi

exec python3 "${SCRIPT_DIR}/harness.py" "$@"
