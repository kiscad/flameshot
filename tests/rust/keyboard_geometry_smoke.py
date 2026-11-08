#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check keyboard selection geometry against the Qt contract.
Usage: keyboard_geometry_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-keyboard-geometry-') as directory:
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
import subprocess,sys,time,os
from pathlib import Path
binary,xdotool,artifacts=sys.argv[1:]
config=Path(os.environ['XDG_CONFIG_HOME'])/'flameshot/flameshot.ini';config.parent.mkdir(exist_ok=True)
config.write_text('[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\nshowSelectionGeometry=0\n')
def tool(*args):
    result=subprocess.run([xdotool,*args],capture_output=True,timeout=5)
    assert result.returncode==0,result.stderr
    return result.stdout.decode()
cases=[
 ('10x10+0+300','Left','9x10+0+300'),
 ('10x10+1014+300','Right','9x10+1015+300'),
 ('10x10+300+0','Up','10x9+300+0'),
 ('10x10+300+758','Down','10x9+300+759'),
 ('1x4+0+300','Left','1x1+0+0'),
 ('1x4+300+300','ctrl+shift+Left','1x4+300+300'),
 ('2x4+300+300','ctrl+shift+Left','1x4+301+300'),
 ('4x1+300+300','ctrl+shift+Down','4x1+300+300'),
]
for region,key,expected in cases:
    process=subprocess.Popen([binary,'gui','--region',region,'--print-geometry'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        deadline=time.monotonic()+15
        while True:
            found=subprocess.run([xdotool,'search','--onlyvisible','--name','^Flameshot$'],capture_output=True)
            if found.returncode==0:break
            assert process.poll() is None,process.communicate()
            assert time.monotonic()<deadline
            time.sleep(.1)
        window=found.stdout.decode().splitlines()[-1]
        tool('windowsize',window,'1024','768');tool('windowfocus','--sync',window);time.sleep(.4)
        tool('key',key);time.sleep(.15);tool('key','Return')
        out,err=process.communicate(timeout=10)
        assert process.returncode==0,err.decode()
        assert out.decode().strip()==expected,(region,key,out,err)
    finally:
        if process.poll() is None:process.kill();process.wait()
print('PASS: keyboard movement clips all four image edges and preserves Qt minimum/symmetric resize behavior')

'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=75)
    finally:
        display.terminate();display.wait(timeout=5)
