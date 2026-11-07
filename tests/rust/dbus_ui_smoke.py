#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Build and run private D-Bus/native UI integration fixtures. Usage: dbus_ui_smoke.py TOOL_ROOT [--documents]"""
import json
from pathlib import Path
import subprocess
import sys

root = str(Path(sys.argv[1]).resolve())
build = subprocess.run(['cargo','test','--locked','-p','flameshot','--all-features',
                        '--test','daemon','--test','portal_open','--no-run','--message-format=json'],
                       capture_output=True, text=True)
if build.returncode:
    print(build.stdout)
    print(build.stderr, file=sys.stderr)
    raise SystemExit(build.returncode)
executables = {}
for line in build.stdout.splitlines():
    message = json.loads(line)
    if message.get('executable'):
        executables[message['target']['name']] = message['executable']
for name, script in [('daemon','tray_smoke.py'), ('portal_open','portal_open_smoke.py')]:
    command = [sys.executable,str(Path(__file__).with_name(script)),root,executables[name]]
    if name == 'portal_open' and '--documents' in sys.argv[2:]:
        command.append('--documents')
    subprocess.run(command,check=True)
