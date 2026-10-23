#!/usr/bin/env python3
"""Consolidate release sources and metadata before removing redundant assets."""
import hashlib
import json
import subprocess
import tarfile
import tempfile
from pathlib import Path


def gh(*args):
    return subprocess.check_output(['gh', *args], text=True)


for tag in ['v0.2.0', 'v0.2.1']:
    release = json.loads(gh('release', 'view', tag, '--json', 'assets,body'))
    assets = release['assets']
    installers = [a for a in assets if a['name'].endswith(('.deb', '.rpm', '.AppImage'))]
    assert len(installers) == 6, 'Expected six installers; refusing cleanup'
    name = f'retoolkit-linux-{tag[1:]}-source-complete.tar.gz'
    extras = [a for a in assets if a not in installers and a['name'] != name]
    if not extras:
        assert any(a['name'] == name for a in assets)
        continue
    with tempfile.TemporaryDirectory() as temp:
        work = Path(temp)
        sources = work / f'retoolkit-linux-{tag[1:]}-source'
        sources.mkdir()
        for asset in extras:
            gh('release', 'download', tag, '--pattern', asset['name'], '--dir', str(sources))
            path = sources / asset['name']
            assert path.is_file() and path.stat().st_size == asset['size'], path
        archive = work / name
        with tarfile.open(archive, 'w:gz', compresslevel=1) as tar:
            tar.add(sources, arcname=sources.name)
        with tarfile.open(archive) as tar:
            members = tar.getnames()
            assert all(f'{sources.name}/{a["name"]}' in members for a in extras)
        gh('release', 'upload', tag, str(archive), '--clobber')
        uploaded = json.loads(gh('release', 'view', tag, '--json', 'assets'))['assets']
        bundle = next(a for a in uploaded if a['name'] == name)
        assert bundle['state'] == 'uploaded' and bundle['size'] == archive.stat().st_size
        # All removed content remains available in the verified, uploaded bundle.
        for asset in extras:
            gh('release', 'delete-asset', tag, asset['name'], '--yes')
    body = release['body']
    start = body.find('## Integridade e fontes')
    if start != -1:
        body = body[:start] + '''## Fontes e integridade

O arquivo `source-complete.tar.gz` reúne os fontes do REtoolkit e das ferramentas incluídas, além das licenças, manifestos e checksums. Extraia esse pacote para consultar os arquivos correspondentes. Os instaladores estão disponíveis individualmente nos anexos.
'''
        with tempfile.TemporaryDirectory() as temp:
            notes = Path(temp) / 'notes.md'
            notes.write_text(body)
            gh('release', 'edit', tag, '--notes-file', str(notes))
    final = json.loads(gh('release', 'view', tag, '--json', 'assets'))['assets']
    assert len(final) == 7
    print(tag, 'cleaned: six installers and one complete source bundle', flush=True)
