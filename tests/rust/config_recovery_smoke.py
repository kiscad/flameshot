#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check invalid-configuration repair on private Xvfb.
Usage: config_recovery_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-config-recovery-') as directory:
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
original='; preserve comment\n[General]\ncheckForUpdates=false\nshowHelp=broken\n[External]\nopaque=@Variant(keep)\n'
config.write_text(original)
def tool(*args):
    result=subprocess.run([xdotool,*args],capture_output=True,timeout=8)
    assert result.returncode==0,result.stderr
    return result.stdout.decode()
def wait_window(process):
    deadline=time.monotonic()+10
    while time.monotonic()<deadline:
        result=subprocess.run([xdotool,'search','--onlyvisible','--name','^Flameshot Configuration$'],capture_output=True)
        if result.returncode==0:
            identifier=result.stdout.decode().splitlines()[-1]
            tool('windowfocus','--sync',identifier);time.sleep(.5)
            return identifier
        assert process.poll() is None,process.communicate()
        time.sleep(.1)
    raise AssertionError('Missing configuration recovery window')
def replace(text):
    tool('key','ctrl+a')
    for index,line in enumerate(text.split('\n')):
        if index:tool('key','Return')
        if line:tool('type','--clearmodifiers','--delay','5',line)
    tool('key','ctrl+s');time.sleep(.4)
for cancel in [True,False]:
    process=subprocess.Popen([binary,'config'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        wait_window(process)
        if cancel:
            tool('key','Escape');out,err=process.communicate(timeout=10)
            assert process.returncode==0,err.decode()
            assert config.read_text()==original
            continue
        replace(original)
        assert config.read_text()==original,('Invalid draft was saved',repr(config.read_text()))
        external=original.replace('; preserve comment','; external comment')
        config.write_text(external)
        replace(original.replace('broken','true'))
        assert config.read_text()==external,'Stale editor overwrote external changes'
        tool('key','F5');time.sleep(.4)
        repaired=external.replace('broken','true')
        replace(repaired)
        deadline=time.monotonic()+5
        while config.read_text()!=repaired:
            assert time.monotonic()<deadline,config.read_text()
            time.sleep(.1)
        check=subprocess.run([binary,'config','--check'],capture_output=True)
        assert check.returncode==0,check.stderr
        # A clean normal form must also enter recovery after an invalid external edit.
        config.write_text(original);time.sleep(1.5)
        replace(repaired)
        assert config.read_text()==repaired,'Live configuration error did not enter recovery'
        tool('key','Escape');out,err=process.communicate(timeout=10)
        assert process.returncode==0,err.decode()
    finally:
        if process.poll() is None:process.kill();process.wait()
print('PASS: invalid configuration opens repair UI; cancel, validation, conflict/reload, exact save and live-error recovery')
'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=75)
    finally:
        display.terminate();display.wait(timeout=5)
