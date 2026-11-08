#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Build the test-only native network fixtures. Usage: network_ui_smoke.py TOOL_ROOT"""
import json
from pathlib import Path
import subprocess
import sys

root = str(Path(sys.argv[1]).resolve())
build = subprocess.run(['cargo','test','--locked','-p','flameshot','--all-features',
                        '--bin','flameshot','--no-run','--message-format=json'],
                       capture_output=True,text=True)
if build.returncode:
    print(build.stdout)
    print(build.stderr,file=sys.stderr)
    raise SystemExit(build.returncode)
executables = [message['executable'] for line in build.stdout.splitlines()
               if (message := json.loads(line)).get('executable')
               and message.get('profile',{}).get('test')
               and message.get('target',{}).get('name') == 'flameshot']
assert len(executables) == 1, executables
for fixture in ['updates_smoke.py','upload_smoke.py']:
    subprocess.run([sys.executable,str(Path(__file__).with_name(fixture)),root,executables[0]],check=True)
