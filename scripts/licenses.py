#!/usr/bin/env python3
"""Retain notices for Cargo dependencies in binary distributions."""
import json,os,shutil,subprocess
from pathlib import Path
root=Path(__file__).resolve().parent.parent
env=os.environ.copy();cargo='cargo'
if (root/'.build/cargo/bin/cargo').exists():
    cargo=str(root/'.build/cargo/bin/cargo');env.update(CARGO_HOME=str(root/'.build/cargo'),RUSTUP_HOME=str(root/'.build/rustup'))
metadata=json.loads(subprocess.check_output([cargo,'metadata','--locked','--offline','--format-version','1'],cwd=root,env=env))
output=root/'.build/licenses';output.mkdir(parents=True,exist_ok=True)
index=[]
for package in metadata['packages']:
    if package['name']=='retoolkit-linux':continue
    src=Path(package['manifest_path']).parent;dst=output/f'{package["name"]}-{package["version"]}';dst.mkdir(exist_ok=True)
    files=[p for p in src.iterdir() if p.is_file() and p.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','COPYRIGHT'))]
    if package.get('license_file'):
        p=src/package['license_file']
        if p.is_file() and p not in files:files.append(p)
    for p in files:shutil.copy2(p,dst/p.name)
    index.append({'name':package['name'],'version':package['version'],'license':package.get('license'),'source':f'https://crates.io/crates/{package["name"]}/{package["version"]}','repository':package.get('repository'),'notice_files':[p.name for p in files]})
(output/'index.json').write_text(json.dumps(index,indent=2)+'\n')
(output/'README.txt').write_text('Cargo dependency license notices. Includes build/platform dependencies as a conservative superset. Source archives are available from the exact crates.io links in index.json. These dependencies retain their original licenses.\n')
print(f'Prepared notices for {len(index)} Cargo packages')
