#!/usr/bin/env python3
"""Create a verifiable release selection index; never publish anything."""
import hashlib,json,re,shutil
from pathlib import Path
root=Path(__file__).resolve().parent.parent
dist=root/'dist';version=re.search(r'^version = "([^"]+)"',(root/'Cargo.toml').read_text(),re.M)[1]
profiles=json.loads((root/'packaging/editions/profiles.json').read_text())
release={'version':version,'published':False,'baseline':'Ubuntu 24.04 x86_64 / glibc >= 2.39','editions':{}}
rows=[]
for edition in ['light','full']:
    folder=dist/edition;title=edition.title()
    names=[f'retoolkit-linux-{edition}_{version}_amd64.deb',f'retoolkit-linux-{edition}-{version}.x86_64.rpm',f'REtoolkit-Linux-{title}-{version}-x86_64.AppImage']
    artifacts=[]
    for name in names:
        path=folder/name
        if not path.is_file() or path.stat().st_size<100_000:raise RuntimeError(f'Missing or incomplete artifact: {path}')
        with path.open('rb') as f:digest=hashlib.file_digest(f,'sha256').hexdigest()
        artifacts.append({'file':name,'bytes':path.stat().st_size,'sha256':digest})
        rows.append(f'| {title} | [{name}]({edition}/{name}) | {path.stat().st_size/(1024*1024):.1f} MiB |')
    companions=[]
    for path in sorted((folder/'sources').glob('*')):
        if path.is_file():
            with path.open('rb') as f:digest=hashlib.file_digest(f,'sha256').hexdigest()
            companions.append({'file':str(path.relative_to(folder)),'bytes':path.stat().st_size,'sha256':digest})
    release['editions'][edition]={'bundled_tools':profiles[edition]['bundled_tools'],'native_deb_tools':profiles[edition]['deb_tools'],'native_rpm_tools':profiles[edition]['rpm_tools'],'artifacts':artifacts,'source_companions':companions}
    (folder/'SHA256SUMS').write_text(''.join(f'{a["sha256"]}  {a["file"]}\n' for a in artifacts+companions))
    (folder/'README.txt').write_text(f'REtoolkit Linux {title} {version}\n'+('Application and catalog; tools are installed on demand.\n' if edition=='light' else 'Bundled: Ghidra 12.2, Cutter 2.5.0, radare2 6.2.4 and Java 25. Native DEB/RPM additionally resolve the basic tools listed in release-manifest.json; AppImage does not install APT/DNF packages.\nKeep and distribute the corresponding source archives in sources/ and the original license notices.\n')+'Validate: sha256sum -c SHA256SUMS\nNot certified on clean Fedora or Ubuntu 26.04. See ../VALIDATION-0.2.0.md.\n')
source=dist/f'retoolkit-linux-{version}-source.tar.gz'
if source.is_file():
    with source.open('rb') as f:digest=hashlib.file_digest(f,'sha256').hexdigest()
    release['application_source']={'file':source.name,'bytes':source.stat().st_size,'sha256':digest}
(dist/'release-manifest.json').write_text(json.dumps(release,indent=2)+'\n')
(dist/'RELEASES.md').write_text(f'''# REtoolkit Linux {version} — escolha sua edição

- **Light**: aplicativo e catálogo, ferramentas sob demanda.
- **Full**: Ghidra, Cutter, radare2 e Java incluídos. DEB/RPM também instalam as ferramentas básicas declaradas; o AppImage mantém essas ferramentas adicionais sob demanda.

Instale apenas uma edição nativa. Use `sudo apt install ./light/...deb` ou `sudo apt install ./full/...deb`, conforme o arquivo escolhido. No AppImage, use `APPIMAGE_EXTRACT_AND_RUN=1` se não houver FUSE.

| Edição | Arquivo | Tamanho |
|---|---|---|
'''+ '\n'.join(rows)+'''

Os arquivos-fonte correspondentes ficam em `full/sources/`. Os checksums estão em cada pasta e no inventário `release-manifest.json`. Nada foi publicado automaticamente. RPM/Fedora e instalações em sistemas limpos ainda exigem validação própria. Arquivos 0.1.0 na raiz foram preservados como histórico.
''')
print('Verified release index:',dist/'RELEASES.md')
