#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check native update-window interactions with local test endpoints on private Xvfb.
Usage: updates_smoke.py TOOL_ROOT APP_TEST_EXECUTABLE
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-native-updates-') as directory:
    env=dict(os.environ,XDG_RUNTIME_DIR=directory,XDG_CONFIG_HOME=directory,XDG_CACHE_HOME=directory,
             XDG_DATA_HOME=directory,XDG_SESSION_TYPE='x11',LC_ALL='C',
             LD_LIBRARY_PATH=str(root/'usr/lib/x86_64-linux-gnu')+':'+os.environ.get('LD_LIBRARY_PATH',''))
    env.update(FLAMESHOT_TEST_PRIVATE_BUS='1',FLAMESHOT_TEST_NATIVE_UPDATES='1',FLAMESHOT_TEST_NATIVE_STATE=str(Path(directory)/'state'))
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
state=Path(os.environ['FLAMESHOT_TEST_NATIVE_STATE'])
name='updates_ui::tests::native_update_window_uses_only_loopback_and_persists_ignore'
process=subprocess.Popen([binary,'--ignored','--exact',name,'--nocapture'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
def tool(*args):
    result=subprocess.run([xdotool,*args],capture_output=True,timeout=5)
    assert result.returncode==0,result.stderr
    return result.stdout.decode()
def phase(expected):
    deadline=time.monotonic()+15
    while time.monotonic()<deadline:
        assert process.poll() is None,process.communicate()
        if state.exists():
            data=state.read_text().splitlines()
            if len(data)==3 and data[0]==expected:return data
        time.sleep(.05)
    raise AssertionError(('Missing phase',expected,state.read_text() if state.exists() else 'no state'))
try:
    first=phase('failed')
    deadline=time.monotonic()+10
    while True:
        result=subprocess.run([xdotool,'search','--onlyvisible','--name','^Flameshot Updates$'],capture_output=True)
        if result.returncode==0:
            window=result.stdout.decode().splitlines()[-1];break
        assert process.poll() is None,process.communicate()
        assert time.monotonic()<deadline,'Update window was not mapped'
        time.sleep(.05)
    tool('windowfocus','--sync',window)
    def click(point):
        x,y=map(float,point.split())
        tool('mousemove','--window',window,str(round(x)),str(round(y)));tool('click','1')
    click(first[2])
    available=phase('available')
    click(available[1])
    ignored=phase('ignored')
    click(ignored[2])
    phase('rechecked')
    tool('key','Escape');out,err=process.communicate(timeout=10)
    assert process.returncode==0,err.decode()
    assert name in out.decode() and 'test result: ok. 1 passed;' in out.decode(),out.decode()
    print(out.decode(),end='')
    print('PASS: native update failure/retry, ignore, recheck, stale-release clearing and close against loopback only')
finally:
    if process.poll() is None:process.kill();process.wait()

'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=75)
    finally:
        display.terminate();display.wait(timeout=5)
