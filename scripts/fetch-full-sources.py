#!/usr/bin/env python3
import hashlib,json,shutil,subprocess
from pathlib import Path
root=Path(__file__).resolve().parent.parent
cache=root/'.build/full-source-archives';cache.mkdir(parents=True,exist_ok=True)
out=root/'dist/full/sources';out.mkdir(parents=True,exist_ok=True)
for item in json.loads((root/'packaging/editions/full-source-archives.json').read_text()):
    target=cache/item['filename']
    def valid():return target.is_file() and hashlib.file_digest(target.open('rb'),'sha256').hexdigest()==item['sha256']
    if not valid():subprocess.run(['curl','--fail','--location','--proto','=https','--proto-redir','=https',item['url'],'--output',str(target)],check=True)
    if not valid():raise RuntimeError('Source archive checksum mismatch')
    shutil.copy2(target,out/target.name);print('VERIFIED SOURCE',item['id'],flush=True)
shutil.copy2(root/'packaging/editions/full-source-archives.json',out/'manifest.json')
