#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh
python3 scripts/licenses.py
python3 scripts/package-editions.py "$@"
