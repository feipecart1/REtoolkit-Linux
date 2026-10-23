#!/usr/bin/env python3
"""Collect installers and corresponding source archives for a GitHub release."""
import hashlib
import json
import re
import shutil
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parent.parent
version = re.search(r'^version = "([^"]+)"', (root / 'Cargo.toml').read_text(), re.M)[1]
out = root / 'dist/release'
out.mkdir(parents=True, exist_ok=False)
manifest = json.loads((root / 'dist/release-manifest.json').read_text())
for edition, info in manifest['editions'].items():
    for item in info['artifacts'] + info['source_companions']:
        source = root / 'dist' / edition / item['file']
        with source.open('rb') as stream:
            assert hashlib.file_digest(stream, 'sha256').hexdigest() == item['sha256'], source
        target = out / source.name
        if target.exists():
            raise RuntimeError(f'Duplicate release filename: {target.name}')
        shutil.copy2(source, target)
subprocess.run(['git', 'archive', '--format=tar.gz', f'--output={out}/retoolkit-linux-{version}-source.tar.gz', 'HEAD'], cwd=root, check=True)
for name in ['LICENSE', 'VALIDATION-0.2.0.md']:
    shutil.copy2(root / name, out / name)
shutil.copy2(root / 'dist/release-manifest.json', out / 'release-manifest.json')
with (out / 'SHA256SUMS').open('w') as checksums:
    for path in sorted(out.iterdir()):
        if path.name == 'SHA256SUMS':
            continue
        with path.open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        checksums.write(f'{digest}  {path.name}\n')
print(f'Release assets verified: {out}')
