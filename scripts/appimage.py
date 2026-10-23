#!/usr/bin/env python3
"""Create separate Light and Full AppImages with explicit content manifests."""
import argparse,hashlib,json,os,re,shutil,subprocess,tempfile
from pathlib import Path
root=Path(__file__).resolve().parent.parent
os.chdir(root)
(root/'.build/editions').mkdir(parents=True,exist_ok=True)
def build(edition,version):
    app=Path(tempfile.mkdtemp(prefix=f'REtoolkit-{edition}.',suffix='.AppDir',dir=root/'.build'))
    for p in ['usr/bin','usr/lib','usr/share/glib-2.0/schemas','usr/share/icons','usr/share/licenses']:(app/p).mkdir(parents=True,exist_ok=True)
    shutil.copy2('target/release/retoolkit-linux',app/'usr/bin/retoolkit-linux')
    share=app/'usr/share/retoolkit-linux';share.mkdir(parents=True)
    profiles=json.loads((root/'packaging/editions/profiles.json').read_text())
    metadata=dict(schema_version=1,edition=edition,version=version,bundled_tools=profiles[edition]['bundled_tools'],distribution_tools=[])
    (share/'edition.json').write_text(json.dumps(metadata,indent=2)+'\n')
    if edition=='full':
        payload=app/'usr/lib/retoolkit-linux/full';payload.parent.mkdir(parents=True,exist_ok=True)
        shutil.copytree(root/'.build/full-payload',payload,symlinks=True)
    
    shutil.copy2('packaging/appimage/AppRun',app/'AppRun')
    (app/'io.github.retoolkitlinux.REtoolkit.desktop').write_text((root/'packaging/desktop/io.github.retoolkitlinux.REtoolkit.desktop').read_text().replace('Name=REtoolkit Linux\n',f'Name=REtoolkit Linux {edition.title()}\n'))
    icon = root/'assets/icons'/('retoolkit-linux-full.svg' if edition == 'full' else 'retoolkit-linux.svg')
    shutil.copy2(icon,app/'retoolkit-linux.svg')
    shutil.copy2(icon,app/'.DirIcon')
    shutil.copy2('LICENSE',app/'usr/share/licenses/REtoolkit-Linux-MIT')
    (app/'usr/share/metainfo').mkdir(exist_ok=True)
    shutil.copy2('packaging/desktop/io.github.retoolkitlinux.REtoolkit.metainfo.xml',app/'usr/share/metainfo/io.github.retoolkitlinux.REtoolkit.metainfo.xml')
    shutil.copytree('.build/licenses',app/'usr/share/licenses/cargo',dirs_exist_ok=True)
    # The host glibc and graphics driver stack remain system components.
    exclude=re.compile(r'^(ld-linux|lib(c|m|dl|pthread|rt|resolv|nss_[^.]+)\.so)')
    libs={}
    def dependencies(path):
        data=subprocess.check_output(['ldd',str(path)],text=True)
        if 'not found' in data:raise RuntimeError(data)
        for line in data.splitlines():
            match=re.search(r'=> (/[^ ]+)',line)
            if match:
                p=Path(match[1]);name=p.name
                if not exclude.match(name):libs[name]=p
    # ldd already resolves the entire transitive dependency closure.
    dependencies('target/release/retoolkit-linux')
    for name,p in libs.items():
        shutil.copy2(p.resolve(),app/'usr/lib'/name)
        ownership=subprocess.run(['dpkg-query','-S',str(p)],capture_output=True,text=True).stdout
        if not ownership:ownership=subprocess.run(['dpkg-query','-S',str(p.resolve())],capture_output=True,text=True).stdout
        for line in ownership.splitlines():
            package=line.split(': ')[0].split(':')[0]
            license=Path('/usr/share/doc')/package/'copyright'
            if license.is_file():shutil.copy2(license,app/'usr/share/licenses'/f'{package}-copyright')
    for license in Path('/usr/share/common-licenses').glob('*'):
        if license.is_file():shutil.copy2(license,app/'usr/share/licenses'/license.name)
    # GTK symbolic icons and GSettings schemas are runtime data, not tool payloads.
    for theme in ['Adwaita','hicolor']:
        src=Path('/usr/share/icons')/theme
        if src.is_dir():shutil.copytree(src,app/'usr/share/icons'/theme,dirs_exist_ok=True,symlinks=False)
    for p in Path('/usr/share/glib-2.0/schemas').glob('*.xml'):shutil.copy2(p,app/'usr/share/glib-2.0/schemas'/p.name)
    subprocess.run(['glib-compile-schemas',str(app/'usr/share/glib-2.0/schemas')],check=True)
    metadata=json.loads(Path('packaging/appimage/build-tools.json').read_text())
    for name,target in [('appimagetool','.build/appimagetool.AppImage'),('runtime','.build/runtime-x86_64')]:
        digest='sha256:'+hashlib.sha256(Path(target).read_bytes()).hexdigest()
        if digest!=metadata[name]['sha256']:raise RuntimeError(f'{name} SHA-256 mismatch')
    Path('.build/appimage-build-tools.json').write_text(json.dumps(metadata,indent=2))
    os.chmod('.build/appimagetool.AppImage',0o755)
    env=dict(os.environ,ARCH='x86_64',APPIMAGE_EXTRACT_AND_RUN='1')
    output=root/f'dist/{edition}/REtoolkit-Linux-{edition.title()}-{version}-x86_64.AppImage'
    temporary=output.with_name('.'+output.stem+'.building.AppImage')
    subprocess.run([str(root/'.build/appimagetool.AppImage'),'--runtime-file',str(root/'.build/runtime-x86_64'),str(app),str(temporary)],check=True,env=env)
    temporary.replace(output)
    shutil.copy2('.build/appimage-build-tools.json',f'dist/{edition}/appimage-build-tools.json')
    (root/f'.build/editions/{edition}-appdir.txt').write_text(str(app)+'\n')
    print(f'{edition.title()}: bundled {len(libs)} shared libraries. Baseline glibc 2.39; only Ubuntu 24.04 tested.')

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--edition',choices=['light','full','all'],default='all');args=parser.parse_args()
    version=re.search(r'^version = "([^"]+)"',(root/'Cargo.toml').read_text(),re.M)[1]
    for edition in (['light','full'] if args.edition=='all' else [args.edition]):
        (root/f'dist/{edition}').mkdir(parents=True,exist_ok=True)
        build(edition,version)
