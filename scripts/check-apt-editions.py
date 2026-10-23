#!/usr/bin/env python3
"""Check APT replacement plans using private copies of package status; no writes to host."""
import re,subprocess,tempfile
from pathlib import Path
root=Path(__file__).resolve().parent.parent
version=re.search(r'^version = "([^"]+)"',(root/'Cargo.toml').read_text(),re.M)[1]
work=Path(tempfile.mkdtemp(prefix='apt-editions.',dir=root/'.build'))
original=Path('/var/lib/dpkg/status').read_text()
blocks=[b for b in original.split('\n\n') if not any(b.startswith('Package: '+name+'\n') for name in ['retoolkit-linux','retoolkit-linux-light','retoolkit-linux-full'])]
for installed,target in [('light','full'),('full','light')]:
    source=root/f'dist/{installed}/retoolkit-linux-{installed}_{version}_amd64.deb'
    control=subprocess.check_output(['dpkg-deb','--field',str(source)],text=True)
    lines=control.splitlines();lines.insert(1,'Status: install ok installed')
    installed_blocks=list(blocks)
    if installed=='full':
        # A completed Full installation includes the dependencies APT would add.
        plan=subprocess.check_output(['apt-get','--simulate','install',str(source)],text=True)
        for package in re.findall(r'^Inst (\S+) ',plan,re.M):
            if package.startswith('retoolkit-linux'):continue
            data=subprocess.check_output(['apt-cache','show','--no-all-versions',package],text=True).split('\n\n')[0]
            fields={}
            for line in data.splitlines():
                if ': ' in line and not line.startswith(' '):
                    key,value=line.split(': ',1);fields[key]=value
            text=f'Package: {fields["Package"]}\nStatus: install ok installed\nVersion: {fields["Version"]}\nArchitecture: {fields["Architecture"]}\n'
            for key in ['Depends','Pre-Depends','Provides','Conflicts','Breaks','Replaces','Multi-Arch']:
                if key in fields:text+=f'{key}: {fields[key]}\n'
            installed_blocks.append(text.strip())
    status=work/f'{installed}.status';status.write_text('\n\n'.join(installed_blocks)+'\n\n'+'\n'.join(lines)+'\n\n')
    package=root/f'dist/{target}/retoolkit-linux-{target}_{version}_amd64.deb'
    out=subprocess.run(['apt-get','--simulate','-o',f'Dir::State::status={status}','install',str(package)],capture_output=True,text=True,check=True)
    (work/f'{installed}-to-{target}.log').write_text(out.stdout+out.stderr)
    assert f'Remv retoolkit-linux-{installed}' in out.stdout
    assert f'Inst retoolkit-linux-{target}' in out.stdout
    print(f'PASS APT {installed} -> {target} replacement plan',flush=True)
print('Evidence:',work)
