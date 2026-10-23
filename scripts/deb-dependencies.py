#!/usr/bin/env python3
"""Combine ELF-derived dependency versions with the supported API baseline."""
import re,subprocess,sys
text=sys.argv[1]
for name,floor in [('libgtk-4-1','4.10'),('libadwaita-1-0','1.4')]:
    pattern=re.compile(re.escape(name)+r' \(>= ([^)]+)\)')
    match=pattern.search(text)
    if match:
        if subprocess.run(['dpkg','--compare-versions',match[1],'lt',floor]).returncode==0:
            text=pattern.sub(f'{name} (>= {floor})',text)
    else:text+=f', {name} (>= {floor})'
print(text)
