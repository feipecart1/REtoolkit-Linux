#!/usr/bin/env python3
"""Run packaging with public failure annotations, without exposing credentials."""
import collections
import subprocess
import sys

commands = [
    ['python3', 'scripts/fetch-full.py'],
    ['python3', 'scripts/prepare-full.py'],
    ['python3', 'scripts/fetch-full-sources.py'],
    ['bash', 'scripts/package.sh'],
    ['python3', 'scripts/fetch-appimage-tools.py'],
    ['python3', 'scripts/appimage.py'],
    ['bash', 'scripts/check-packages.sh'],
    ['python3', 'scripts/release-index.py'],
]
for command in commands:
    print('::group::' + ' '.join(command), flush=True)
    tail = collections.deque(maxlen=20)
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    for line in process.stdout:
        print(line, end='', flush=True)
        tail.append(line.rstrip())
    status = process.wait()
    print('::endgroup::', flush=True)
    if status:
        message = (' '.join(command) + '\n' + '\n'.join(tail)).replace('%', '%25').replace('\r', '%0D').replace('\n', '%0A')
        print('::error::' + message, flush=True)
        sys.exit(status)
