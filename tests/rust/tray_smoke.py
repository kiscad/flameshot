#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Exercise tray monitor activation on a private two-monitor Xvfb desktop.
Usage: tray_smoke.py TOOL_ROOT DAEMON_TEST_EXECUTABLE
Build the test executable with cargo test --test daemon --no-run first.
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root, test = (Path(p).resolve() for p in sys.argv[1:])
with tempfile.TemporaryDirectory(prefix='flameshot-tray-') as directory:
    env = dict(os.environ, XDG_RUNTIME_DIR=directory, XDG_CACHE_HOME=directory, XDG_CONFIG_HOME=directory, XDG_SESSION_TYPE='x11',
               FLAMESHOT_TEST_PRIVATE_BUS='1', FLAMESHOT_TEST_PRIVATE_DISPLAY_TOOLS=str(root),
               LD_LIBRARY_PATH=str(root/'usr/lib/x86_64-linux-gnu')+':'+os.environ.get('LD_LIBRARY_PATH',''))
    env.pop('WAYLAND_DISPLAY', None)
    bus_config = Path(directory)/'bus.conf'
    bus_config.write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
    with tempfile.TemporaryFile() as log:
        display = subprocess.Popen([str(root/'usr/bin/Xvfb'), '-displayfd', '1', '-screen', '0', '1024x768x24', '-nolisten', 'tcp', '-noreset'], stdout=subprocess.PIPE, stderr=log, env=env)
        try:
            number = display.stdout.readline().decode().strip()
            assert number, 'Xvfb failed to start'
            env['DISPLAY'] = ':'+number
            for name, bounds, output in [('left','512/135x768/203+0+0','screen'),('right','512/135x768/203+512+0','none')]:
                subprocess.run(['xrandr','--setmonitor',name,bounds,output],env=env,check=True,capture_output=True,timeout=5)
            bitmap = Path(directory)/'monitors.xbm'
            bitmap.write_text('#define colors_width 1024\n#define colors_height 1\nstatic unsigned char colors_bits[] = {\n'+','.join(['0xff']*64+['0x00']*64)+'};\n')
            subprocess.run(['xsetroot','-fg','blue','-bg','red','-bitmap',str(bitmap)],env=env,check=True,timeout=5)
            subprocess.run(['dbus-run-session','--config-file',str(bus_config),'--',str(test),'--ignored','--nocapture'],env=env,check=True,timeout=40)
            print('PASS: actual tray monitor selection copies the selected red 512x768 output and refreshes after removal')
        finally:
            display.terminate()
            display.wait(timeout=5)
