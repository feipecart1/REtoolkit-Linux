#!/usr/bin/env python3
"""Download pinned official tool releases without installing on the build host."""
import hashlib,json,subprocess
from pathlib import Path
root=Path(__file__).resolve().parent.parent
manifest=json.loads((root/'packaging/editions/full-sources.json').read_text())
cache=root/'.build/full-downloads';cache.mkdir(exist_ok=True)
for item in manifest['tools']:
    target=cache/item['filename']
    def valid():return target.is_file() and hashlib.file_digest(target.open('rb'),'sha256').hexdigest()==item['sha256']
    if not valid():
        if not item['url'].startswith('https://github.com/'):raise ValueError('Unapproved source')
        subprocess.run(['curl','--fail','--location','--retry','2','--proto','=https','--proto-redir','=https',item['url'],'--output',str(target)],check=True)
    if not valid():raise RuntimeError(f'SHA-256 mismatch for {item["id"]}; do not use this artifact')
    print(f'VERIFIED {item["id"]} {item["version"]} {item["sha256"]}',flush=True)
