#!/usr/bin/env python3
"""Fetch build tools over HTTPS and verify committed SHA-256 before execution."""
import hashlib,json,subprocess
from pathlib import Path
root=Path(__file__).resolve().parent.parent
pins=json.loads((root/'packaging/appimage/build-tools.json').read_text())
for name,filename in [('appimagetool','appimagetool.AppImage'),('runtime','runtime-x86_64')]:
    pin=pins[name];target=root/'.build'/filename;target.parent.mkdir(exist_ok=True)
    valid=lambda: target.exists() and 'sha256:'+hashlib.sha256(target.read_bytes()).hexdigest()==pin['sha256']
    if not valid():
        subprocess.run(['curl','--fail','--location','--proto','=https','--proto-redir','=https',pin['url'],'--output',str(target)],check=True)
    if not valid():raise RuntimeError(f'{name} checksum mismatch. Do not execute; review the upstream artifact and update the pin explicitly.')
    print(f'{name}: verified {pin["sha256"]}')
