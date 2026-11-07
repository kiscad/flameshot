#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check Chinese launcher rendering and its capture/cancel lifecycle on private Xvfb.
Usage: localization_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-localization-') as directory:
    env=dict(os.environ,XDG_RUNTIME_DIR=directory,XDG_CONFIG_HOME=directory,XDG_CACHE_HOME=directory,
             XDG_DATA_HOME=directory,XDG_SESSION_TYPE='x11',LC_ALL='C',
             LD_LIBRARY_PATH=str(root/'usr/lib/x86_64-linux-gnu')+':'+os.environ.get('LD_LIBRARY_PATH',''))
    env.pop('WAYLAND_DISPLAY',None)
    config=Path(directory)/'bus.conf'
    config.write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
    display=subprocess.Popen([str(root/'usr/bin/Xvfb'),'-displayfd','1','-screen','0','1024x768x24','-nolisten','tcp','-noreset'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env)
    try:
        number=display.stdout.readline().decode().strip()
        assert number,'Xvfb did not start'
        env['DISPLAY']=':'+number
        driver=r'''
import subprocess,sys,time,os,re
from pathlib import Path
binary,xdotool,artifacts=sys.argv[1:]
config=Path(os.environ['XDG_CONFIG_HOME'])/'flameshot/flameshot.ini'
config.parent.mkdir(exist_ok=True)
def tool(*args):
    return subprocess.run([xdotool,*args],env=dict(os.environ,LC_ALL="C.UTF-8"),capture_output=True,timeout=5)
def window(title):
    deadline=time.monotonic()+10
    while time.monotonic()<deadline:
        found=tool('search','--onlyvisible','--name','^Flameshot')
        for identifier in reversed(found.stdout.decode().splitlines()):
            name=tool('getwindowname',identifier).stdout.decode().rstrip('\n')
            if re.search(title,name):return identifier
        time.sleep(.1)
    names=tool('search','--onlyvisible','--name','Flameshot').stdout.decode().splitlines()
    raise AssertionError('Missing '+title+'; windows: '+repr([tool('getwindowname',name).stdout for name in names]))
for locale,title in [('zh_CN','Flameshot 启动器'),('zh_TW','Flameshot 啟動器')]:
    config.write_text(f'[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\nuiLanguage={locale}\n')
    process=subprocess.Popen([binary,'launcher'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        launcher=window('^'+title+'$')
        tool('windowfocus','--sync',launcher)
        time.sleep(.5)
        captured=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
        assert captured.returncode==0,captured.stderr.decode()
        Path(artifacts,f'rust-launcher-{locale}.png').write_bytes(captured.stdout)
        tool('key','Return')
        capture=window('^Flameshot$')
        tool('windowfocus','--sync',capture)
        time.sleep(.3)
        tool('key','Escape')
        launcher=window('^'+title+'$')
        tool('windowfocus','--sync',launcher)
        time.sleep(.3)
        tool('key','Escape')
        out,err=process.communicate(timeout=10)
        assert process.returncode==0,err.decode()
    finally:
        if process.poll() is None:process.kill()
        out,err=process.communicate()
        if process.returncode:print(err.decode(),file=sys.stderr)
    print('PASS:',locale,'launcher title, capture/cancel lifecycle and rendered screenshot')
'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=50)
    finally:
        display.terminate();display.wait(timeout=5)
