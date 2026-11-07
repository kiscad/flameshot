#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Exercise sandbox Open With, SaveFile and web links against private portals on Xvfb.
Usage: portal_open_smoke.py TOOL_ROOT PORTAL_OPEN_TEST_EXECUTABLE [--documents]
Build the test executable with cargo test --test portal_open --no-run first.
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root, test = (Path(p).resolve() for p in sys.argv[1:3])
with tempfile.TemporaryDirectory(prefix='flameshot-open-portal-') as directory:
    env = dict(os.environ, XDG_RUNTIME_DIR=directory, XDG_DATA_HOME=directory, XDG_CACHE_HOME=directory, XDG_CONFIG_HOME=directory, XDG_SESSION_TYPE='x11',
               FLAMESHOT_TEST_PRIVATE_BUS='1', FLAMESHOT_TEST_PRIVATE_DISPLAY_TOOLS=str(root),
               LD_LIBRARY_PATH=str(root/'usr/lib/x86_64-linux-gnu')+':'+os.environ.get('LD_LIBRARY_PATH',''))
    env.pop('WAYLAND_DISPLAY', None)
    env.pop('FLAMESHOT_TEST_DOCUMENT_PORTAL', None)
    bus_config = Path(directory)/'bus.conf'
    bus_config.write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
    with tempfile.TemporaryFile() as log:
        display = subprocess.Popen([str(root/'usr/bin/Xvfb'), '-displayfd', '1', '-screen', '0', '1024x768x24', '-nolisten', 'tcp', '-noreset'], stdout=subprocess.PIPE, stderr=log, env=env)
        try:
            number = display.stdout.readline().decode().strip()
            assert number, 'Xvfb failed to start'
            env['DISPLAY'] = ':'+number
            command = [str(test),'--ignored','--nocapture']
            if '--documents' in sys.argv[3:]:
                env['FLAMESHOT_TEST_DOCUMENT_PORTAL'] = '1'
                driver = r"""
import os,subprocess,sys,time
store=portal=None
try:
    store=subprocess.Popen(['/usr/libexec/xdg-permission-store'])
    portal=subprocess.Popen(['/usr/libexec/xdg-document-portal'])
    deadline=time.monotonic()+5
    while True:
        ready=subprocess.run(['gdbus','call','--session','--dest','org.freedesktop.portal.Documents','--object-path','/org/freedesktop/portal/documents','--method','org.freedesktop.portal.Documents.GetMountPoint'],capture_output=True,timeout=2)
        if ready.returncode==0:break
        assert portal.poll() is None and time.monotonic()<deadline,'Document portal failed to mount'
        time.sleep(.1)
    subprocess.run(sys.argv[1:],check=True,timeout=60)
finally:
    for process in [portal,store]:
        if process is not None:
            process.terminate();process.wait(timeout=5)
    subprocess.run(['/usr/bin/fusermount3','-u',os.environ['XDG_RUNTIME_DIR']+'/doc'],capture_output=True,timeout=5)
"""
                command = [sys.executable,'-c',driver,*command]
            subprocess.run(['dbus-run-session','--config-file',str(bus_config),'--',*command],env=env,check=True,timeout=80)
            print('PASS: sandbox Open With, SaveFile and web links preserve data and handle cancellation/errors')
        finally:
            if '--documents' in sys.argv[3:]:
                subprocess.run(['/usr/bin/fusermount3','-u',str(Path(directory)/'doc')],capture_output=True,timeout=5)
            display.terminate()
            display.wait(timeout=5)
