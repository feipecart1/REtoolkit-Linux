#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh
# License inventory covers platform and build dependencies as well.
cargo fetch --locked
python3 scripts/licenses.py
python3 scripts/package-editions.py "$@"
