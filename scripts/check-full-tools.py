#!/usr/bin/env python3
"""Run only pinned tools and inspect a known system ELF; never run a sample."""
import argparse,json,os,re,signal,subprocess,tempfile,time
from pathlib import Path
root=Path(__file__).resolve().parent.parent
p=argparse.ArgumentParser();p.add_argument('--prefix',type=Path,default=root/'.build/full-payload');p.add_argument('--gui',action='store_true');args=p.parse_args();prefix=args.prefix.resolve()
work=Path(tempfile.mkdtemp(prefix='full-tool-check.',dir=root/'.build'))
env=os.environ.copy();env.update(XDG_CONFIG_HOME=str(work/'config'),XDG_DATA_HOME=str(work/'data'),XDG_CACHE_HOME=str(work/'cache'),_JAVA_OPTIONS=f'-Duser.home={work}/home',QT_QPA_PLATFORM='xcb')
for name in ['config','data','cache','home']:(work/name).mkdir()
java=subprocess.run([str(prefix/'tools/jdk/bin/java'),'-version'],capture_output=True,text=True,env=env,check=True);assert '25.' in java.stderr;print('PASS bundled Java 25',flush=True)
r2=subprocess.run([str(prefix/'bin/radare2'),'-q','-e','bin.relocs.apply=true','-c','iIj;q','/usr/bin/ls'],capture_output=True,text=True,env=env,check=True)
report=json.loads(r2.stdout);assert report['bits']==64 and report['bintype']=='elf';print('PASS bundled radare2 static ELF inspection',flush=True)
headless_env=dict(env,JAVA_HOME=str(prefix/'tools/jdk'),PATH=str(prefix/'tools/jdk/bin')+':'+env['PATH'])
ghidra=subprocess.run([str(prefix/'tools/ghidra/support/analyzeHeadless')],capture_output=True,text=True,env=headless_env,timeout=30);assert 'Headless Analyzer Usage' in ghidra.stdout+ghidra.stderr;print('PASS bundled Ghidra launcher and Java runtime',flush=True)
if args.gui:
    cutter=subprocess.run([str(prefix/'bin/cutter'),'--version'],capture_output=True,text=True,env=env,check=True,timeout=30);assert 'cutter 2.5.0' in cutter.stdout;print('PASS bundled Cutter 2.5.0 version',flush=True)
    for name in ['cutter','ghidra']:
        with (work/f'{name}.log').open('w') as log:
            child=subprocess.Popen([str(prefix/'bin'/name)],stdout=log,stderr=log,env=env,start_new_session=True)
            try:
                start=time.monotonic();found=False
                while time.monotonic()-start<45:
                    windows=subprocess.check_output(['xwininfo','-root','-tree'],text=True)
                    for line in windows.splitlines():
                        if name.lower() not in line.lower():continue
                        match=re.match(r'\s*(0x[0-9a-f]+)',line)
                        if not match:continue
                        state=subprocess.check_output(['xwininfo','-id',match[1]],text=True)
                        width=re.search(r'Width: (\d+)',state);height=re.search(r'Height: (\d+)',state)
                        if 'Map State: IsViewable' in state and width and height and int(width[1])>300 and int(height[1])>200:
                            (work/f'{name}-windows.txt').write_text(windows);found=True;break
                    if found:time.sleep(3);break
                    time.sleep(.3)
                if not found:raise RuntimeError(f'{name} window did not open; see {work}')
                subprocess.run(['import','-window','root',str(work/f'{name}.png')],check=True)
                print(f'PASS bundled {name} GUI startup',flush=True)
            finally:
                try:os.killpg(child.pid,signal.SIGTERM)
                except ProcessLookupError:pass
                try:child.wait(timeout=5)
                except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
print('Evidence:',work,flush=True)
