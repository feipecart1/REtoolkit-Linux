#!/usr/bin/env python3
"""Produce alternative Light/Full packages. No postinst downloads or nested APT."""
import argparse,json,os,re,shutil,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent

def profiles():return json.loads((ROOT/'packaging/editions/profiles.json').read_text())
def version():return re.search(r'^version = "([^"]+)"',(ROOT/'Cargo.toml').read_text(),re.M)[1]
def info(edition,fmt):
    p=profiles()[edition]
    return dict(schema_version=1,edition=edition,version=version(),bundled_tools=p['bundled_tools'],distribution_tools=p.get(fmt+'_tools',[]))
def install(src,dst,mode=None):
    dst.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(src,dst)
    if mode is not None:dst.chmod(mode)
def stage(edition,fmt):
    path=Path(tempfile.mkdtemp(prefix=f'{edition}-{fmt}.',dir=ROOT/'.build/packages'))
    share=path/'usr/share/retoolkit-linux';share.mkdir(parents=True)
    (share/'edition.json').write_text(json.dumps(info(edition,fmt),indent=2)+'\n')
    install(ROOT/'target/release/retoolkit-linux',path/'usr/bin/retoolkit-linux',0o755)
    desktop=(ROOT/'packaging/desktop/io.github.retoolkitlinux.REtoolkit.desktop').read_text().replace('Name=REtoolkit Linux\n',f'Name=REtoolkit Linux {edition.title()}\n')
    target=path/'usr/share/applications/io.github.retoolkitlinux.REtoolkit.desktop';target.parent.mkdir(parents=True);target.write_text(desktop)
    icon = 'retoolkit-linux-full.svg' if edition == 'full' else 'retoolkit-linux.svg'
    install(ROOT/'assets/icons'/icon,path/'usr/share/icons/hicolor/scalable/apps/retoolkit-linux.svg')
    doc=path/f'usr/share/doc/retoolkit-linux-{edition}';doc.mkdir(parents=True)
    install(ROOT/'LICENSE',doc/'copyright');shutil.copytree(ROOT/'.build/licenses',doc/'third-party-licenses')
    if edition=='full':
        payload=path/'usr/lib/retoolkit-linux/full';payload.parent.mkdir(parents=True,exist_ok=True)
        shutil.copytree(ROOT/'.build/full-payload',payload,symlinks=True)
    return path

def dependencies():
    work=ROOT/'.build/shlibdeps';(work/'debian').mkdir(parents=True,exist_ok=True)
    (work/'debian/control').write_text('Source: retoolkit-linux\nSection: devel\nMaintainer: REtoolkit Linux contributors <retoolkit-linux@localhost>\n\n'+(ROOT/'packaging/deb/control').read_text())
    out=subprocess.check_output(['dpkg-shlibdeps','-O','-e',str(ROOT/'target/release/retoolkit-linux')],cwd=work,text=True)
    deps=next(l.removeprefix('shlibs:Depends=') for l in out.splitlines() if l.startswith('shlibs:Depends='))
    return subprocess.check_output(['python3',str(ROOT/'scripts/deb-dependencies.py'),deps],text=True).strip()

def build_deb(edition,deps):
    path=stage(edition,'deb');other='full' if edition=='light' else 'light';name=f'retoolkit-linux-{edition}'
    extra=profiles()[edition]['deb_tools']
    if edition=='full':extra+=['libasound2t64','libfontconfig1','libfreetype6','libx11-6','libxext6','libxi6','libxrender1','libxtst6','libstdc++6','zlib1g']
    all_deps=', '.join([deps]+extra)
    control=f'''Package: {name}
Version: {version()}
Section: devel
Priority: optional
Architecture: amd64
Maintainer: REtoolkit Linux contributors <retoolkit-linux@localhost>
Provides: retoolkit-linux (= {version()})
Conflicts: retoolkit-linux, retoolkit-linux-{other}
Replaces: retoolkit-linux, retoolkit-linux-{other}
Depends: {all_deps}
Recommends: policykit-1, xdg-utils, gnome-terminal
Description: REtoolkit Linux {edition.title()} edition
 Native reverse engineering workstation. {"Includes pinned Ghidra, Cutter, radare2 and Java; base tools are installed by APT." if edition=="full" else "Tools can be installed on demand from the application."}
'''
    (path/'DEBIAN').mkdir();(path/'DEBIAN/control').write_text(control)
    out=ROOT/f'dist/{edition}/{name}_{version()}_amd64.deb';out.parent.mkdir(parents=True,exist_ok=True)
    temporary=out.with_name('.'+out.name+'.building')
    subprocess.run(['dpkg-deb','-Zzstd','-z3','--threads-max=2','--root-owner-group','--build',str(path),str(temporary)],check=True)
    temporary.replace(out)
    if os.environ.get("RETOOLKIT_KEEP_STAGING")!="1":shutil.rmtree(path)
    return out

def build_rpm(edition):
    path=stage(edition,'rpm');other='full' if edition=='light' else 'light';name=f'retoolkit-linux-{edition}'
    extra=profiles()[edition]['rpm_tools']
    if edition=='full':extra+=['alsa-lib','fontconfig','freetype','libX11','libXext','libXi','libXrender','libXtst','libstdc++','zlib']
    requires='\n'.join('Requires: '+s for s in extra)
    licenses='MIT' if edition=='light' else 'MIT AND Apache-2.0 AND GPL-3.0-only AND LGPL-3.0-only AND (GPL-2.0-only WITH Classpath-exception-2.0) AND Python-2.0'
    spec=f'''%global __requires_exclude_from ^/usr/lib/retoolkit-linux/full/.*$
%global __provides_exclude_from ^/usr/lib/retoolkit-linux/full/.*$
Name: {name}
Version: {version()}
Release: 1
Summary: REtoolkit Linux {edition.title()} edition
License: {licenses}
BuildArch: x86_64
Provides: retoolkit-linux = %{{version}}-%{{release}}
Conflicts: retoolkit-linux-{other}
Obsoletes: retoolkit-linux < 0.2.0
Requires: glibc >= 2.39
Requires: gtk4 >= 4.10
Requires: libadwaita >= 1.4
{requires}
Recommends: polkit, xdg-utils, gnome-terminal
%description
Native reverse engineering workstation, {edition.title()} edition. Third-party payloads retain their original licenses in their directories and source manifest.
%install
mkdir -p %{{buildroot}}
cp -a '{path}/usr' %{{buildroot}}/
%files
/usr/bin/retoolkit-linux
/usr/share/applications/io.github.retoolkitlinux.REtoolkit.desktop
/usr/share/icons/hicolor/scalable/apps/retoolkit-linux.svg
/usr/share/doc/{name}
/usr/share/retoolkit-linux
{'/usr/lib/retoolkit-linux' if edition=='full' else ''}
'''
    specpath=ROOT/f'.build/{name}.spec';specpath.write_text(spec)
    for folder in ['rpmbuild','rpm-tmp','rpmdb']:(ROOT/'.build'/folder).mkdir(exist_ok=True)
    subprocess.run(['rpmbuild','--define',f'_topdir {ROOT}/.build/rpmbuild','--define',f'_tmppath {ROOT}/.build/rpm-tmp','--define',f'_dbpath {ROOT}/.build/rpmdb','--define','_binary_payload w3.zstdio','--define','_build_id_links none','--define','__os_install_post %{nil}','-bb',str(specpath)],check=True)
    out=ROOT/f'dist/{edition}/{name}-{version()}.x86_64.rpm';out.parent.mkdir(parents=True,exist_ok=True)
    shutil.copy2(ROOT/f'.build/rpmbuild/RPMS/x86_64/{name}-{version()}-1.x86_64.rpm',out)
    if os.environ.get("RETOOLKIT_KEEP_STAGING")!="1":shutil.rmtree(path)
    return out

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--edition',choices=['light','full','all'],default='all');parser.add_argument('--format',choices=['deb','rpm','all'],default='all');args=parser.parse_args()
    os.chdir(ROOT);(ROOT/'.build/packages').mkdir(exist_ok=True)
    selected=['light','full'] if args.edition=='all' else [args.edition]
    deps=dependencies()
    for edition in selected:
        if edition=='full' and not (ROOT/'.build/full-payload/build.sha256').is_file():raise RuntimeError('Run fetch-full.py and prepare-full.py first')
        (ROOT/f'dist/{edition}').mkdir(parents=True,exist_ok=True)
        if args.format in ['deb','all']:print(build_deb(edition,deps),flush=True)
        if args.format in ['rpm','all']:print(build_rpm(edition),flush=True)
        (ROOT/f'dist/{edition}/edition.json').write_text(json.dumps(info(edition,'deb'),indent=2)+'\n')
if __name__=='__main__':main()
