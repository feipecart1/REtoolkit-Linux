#!/usr/bin/env python3
"""Assemble verified tools in a relocatable, private prefix; never modify the host."""
import hashlib,json,os,shutil,stat,subprocess,tarfile,tempfile,zipfile
from pathlib import Path
root=Path(__file__).resolve().parent.parent
manifest=json.loads((root/'packaging/editions/full-sources.json').read_text())
out=root/'.build/full-payload'
stamp=hashlib.sha256(json.dumps(manifest,sort_keys=True).encode()).hexdigest()
if (out/'build.sha256').is_file():
    if (out/'build.sha256').read_text().strip()==stamp:
        print('Verified full payload already prepared');raise SystemExit(0)
    raise RuntimeError('Existing payload differs. Move it aside before preparing a new version.')
if out.exists():raise RuntimeError('Existing incomplete payload preserved; move it aside first')
work=Path(tempfile.mkdtemp(prefix='full-stage.',dir=root/'.build'))
(work/'tools').mkdir();(work/'bin').mkdir()
for tool in manifest['tools']:
    archive=root/'.build/full-downloads'/tool['filename']
    if hashlib.file_digest(archive.open('rb'),'sha256').hexdigest()!=tool['sha256']:raise RuntimeError(f'Checksum mismatch: {tool["id"]}')
    destination=work/'tools'/tool['id']
    if tool['id']=='ghidra':
        stage=work/'extract-ghidra';stage.mkdir()
        with zipfile.ZipFile(archive) as z:
            for item in z.infolist():
                p=Path(item.filename)
                if p.is_absolute() or '..' in p.parts or stat.S_ISLNK(item.external_attr>>16):raise RuntimeError('Unsafe ZIP member')
                target=Path(z.extract(item,stage));mode=(item.external_attr>>16)&0o777
                if mode:target.chmod(mode)
        children=list(stage.iterdir());assert len(children)==1
        children[0].rename(destination);stage.rmdir()
    elif tool['id']=='cutter':
        archive.chmod(0o755)
        subprocess.run([str(archive),'--appimage-extract'],cwd=work,stdout=subprocess.DEVNULL,check=True)
        (work/'squashfs-root').rename(destination)
    elif tool['id']=='radare2':
        subprocess.run(['dpkg-deb','--extract',str(archive),str(destination)],check=True)
    elif tool['id']=='jdk':
        stage=work/'extract-jdk';stage.mkdir()
        with tarfile.open(archive) as t:t.extractall(stage,filter='data')
        children=list(stage.iterdir());assert len(children)==1
        children[0].rename(destination);stage.rmdir()
    print(f'Prepared {tool["id"]}',flush=True)
common='''#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
'''
wrappers={
'ghidra':'''export JAVA_HOME="$ROOT/tools/jdk"
export PATH="$JAVA_HOME/bin:$PATH"
exec "$ROOT/tools/ghidra/ghidraRun" "$@"
''',
'cutter':'''unset QT_PLUGIN_PATH QT_QPA_PLATFORM_PLUGIN_PATH QML2_IMPORT_PATH
export APPDIR="$ROOT/tools/cutter"
exec "$APPDIR/AppRun" "$@"
''',
'radare2':'''export R2_PREFIX="$ROOT/tools/radare2/usr"
export LD_LIBRARY_PATH="$R2_PREFIX/lib:$R2_PREFIX/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$R2_PREFIX/bin/radare2" "$@"
'''}
for name,body in wrappers.items():
    path=work/'bin'/name;path.write_text(common+body);path.chmod(0o755)
(work/'sources.json').write_text(json.dumps(manifest,indent=2)+'\n')
(work/'build.sha256').write_text(stamp+'\n')
(work/'README.txt').write_text('Pinned official Ghidra, Cutter, radare2 and Temurin JDK. Original license/NOTICE/legal files remain in each tools directory. Sources and exact versions are listed in sources.json. This prefix is private to REtoolkit; no system command is overwritten.\n')
work.rename(out)
print(f'Full payload: {out}')
