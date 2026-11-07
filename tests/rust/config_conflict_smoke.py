#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check conflicting configuration edits on private Xvfb.
Usage: config_conflict_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-config-conflict-') as directory:
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
import subprocess,sys,time,os,io
from pathlib import Path
from PIL import Image
binary,xdotool,artifacts=sys.argv[1:]
config=Path(os.environ['XDG_CONFIG_HOME'])/'flameshot/flameshot.ini';config.parent.mkdir(exist_ok=True)
config.write_text('[General]\ncheckForUpdates=false\nuseX11LegacyScreenshot=true\nfilenamePattern=initial\nprivateValue=keep\n')
def tool(*args):
    result=subprocess.run([xdotool,*args],capture_output=True,timeout=5)
    assert result.returncode==0,result.stderr
    return result.stdout.decode()
def snapshot(name):
    captured=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
    assert captured.returncode==0,captured.stderr
    Path(artifacts,name).write_bytes(captured.stdout)
    return Image.open(io.BytesIO(captured.stdout)).convert('RGB')
process=subprocess.Popen([binary,'config'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
try:
    deadline=time.monotonic()+15
    while True:
        found=subprocess.run([xdotool,'search','--onlyvisible','--name','^Flameshot Configuration$'],capture_output=True)
        if found.returncode==0:break
        assert time.monotonic()<deadline
        time.sleep(.1)
    window=found.stdout.decode().splitlines()[-1]
    tool('windowmove',window,'0','0');tool('windowfocus','--sync',window);time.sleep(.4)
    def click(x,y):
        tool('mousemove','--window',window,str(x),str(y));tool('click','1');time.sleep(.2)
    def scroll(button):
        tool('mousemove','--window',window,'200','300');tool('click','--repeat','60','--delay','10',button);time.sleep(.3)
    click(100,162);tool('key','ctrl+a');tool('type','--clearmodifiers','local-draft');time.sleep(.2)
    result=subprocess.run([binary,'config','--filename','external-value'],capture_output=True,timeout=10)
    assert result.returncode==0 and not result.stderr,result.stderr
    external=config.read_bytes();time.sleep(1.2)
    scroll('5')
    before=snapshot('rust-config-conflict-before.png').crop((0,405,520,420)).tobytes()
    click(45,393)
    after=snapshot('rust-config-conflict-after.png').crop((0,405,520,420)).tobytes()
    assert before!=after,'Conflict status did not appear'
    assert config.read_bytes()==external,'Conflict overwrote external configuration'
    scroll('4');click(100,162);tool('key','ctrl+a','ctrl+c');time.sleep(.2)
    copied=subprocess.run([str(Path(xdotool).with_name('xclip')),'-selection','clipboard','-o'],capture_output=True,timeout=5)
    assert copied.returncode==0 and copied.stdout.decode()=='local-draft',copied.stdout
    scroll('5');snapshot('rust-config-conflict-reload.png')
    click(180,393) # Explicitly discard the draft and reload saved settings.
    assert config.read_bytes()==external,'Reload modified the file'
    scroll('4');click(100,162);tool('key','ctrl+a','ctrl+c');time.sleep(.2)
    copied=subprocess.run([str(Path(xdotool).with_name('xclip')),'-selection','clipboard','-o'],capture_output=True,timeout=5)
    assert copied.returncode==0 and copied.stdout.decode()=='external-value',copied.stdout
    tool('type','--clearmodifiers','resolved-value');time.sleep(.2)
    result=subprocess.run([binary,'config','--notifications','false'],capture_output=True,timeout=10)
    assert result.returncode==0 and not result.stderr,result.stderr
    scroll('5');click(45,393)
    import configparser
    saved=configparser.ConfigParser(interpolation=None);saved.read(config)
    assert saved.get('General','filenamePattern').strip(chr(34))=='resolved-value'
    assert saved.get('General','showDesktopNotification').strip(chr(34))=='false'
    assert saved.get('General','privateValue')=='keep'
    tool('key','Escape');out,err=process.communicate(timeout=10);assert process.returncode==0,err.decode()
    print('PASS: conflict preserves file/draft, explicit reload adopts external values, and retry merges unrelated changes')
finally:
    if process.poll() is None:process.kill();process.wait()
'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=75)
    finally:
        display.terminate();display.wait(timeout=5)
