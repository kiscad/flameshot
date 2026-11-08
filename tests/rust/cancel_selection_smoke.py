#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check selection cancellation and annotation history.
Usage: cancel_selection_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-cancel-selection-') as directory:
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
import subprocess,sys,time,os,io,struct
from pathlib import Path
from PIL import Image
binary,xdotool,artifacts=sys.argv[1:]
config=Path(os.environ['XDG_CONFIG_HOME'])/'flameshot/flameshot.ini';config.parent.mkdir(exist_ok=True)
data=struct.pack('>II',127,11)+b'QList<int>\0'+struct.pack('>I',0)
variant='@Variant('+''.join(chr(92)+'x'+format(byte,'02x') for byte in data)+')'
config.write_text('[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\nshowSelectionGeometry=0\nbuttons='+variant+'\n')
def tool(*args):
    result=subprocess.run([xdotool,*args],capture_output=True,timeout=5)
    assert result.returncode==0,result.stderr
    return result.stdout.decode()
def snapshot(name):
    captured=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
    assert captured.returncode==0,captured.stderr
    Path(artifacts,name).write_bytes(captured.stdout)
    return Image.open(io.BytesIO(captured.stdout)).convert('RGB').getpixel((650,550))
process=subprocess.Popen([binary,'gui','--region','400x200+500+450','--raw'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
try:
    deadline=time.monotonic()+15
    while True:
        found=subprocess.run([xdotool,'search','--onlyvisible','--name','^Flameshot$'],capture_output=True)
        if found.returncode==0:break
        assert process.poll() is None,process.communicate()
        assert time.monotonic()<deadline
        time.sleep(.1)
    window=found.stdout.decode().splitlines()[-1]
    tool('windowsize',window,'1024','768');tool('windowfocus','--sync',window);time.sleep(.5)
    background=snapshot('rust-cancel-background.png')
    tool('key','r');tool('mousemove','--window',window,'600','500');tool('mousedown','1')
    tool('mousemove','--window',window,'700','600');time.sleep(.2);tool('mouseup','1');time.sleep(.3)
    assert snapshot('rust-cancel-annotation.png')==(255,0,0)
    tool('mousemove','--window',window,'180','37');tool('click','1');time.sleep(.2)
    tool('mousemove','--window',window,'650','550');tool('click','1');time.sleep(.2)
    # Prove that the object is selected before exercising cancellation.
    tool('key','Delete');time.sleep(.2)
    assert snapshot('rust-cancel-delete-control.png')==background
    tool('key','ctrl+z');time.sleep(.2)
    assert snapshot('rust-cancel-restored.png')==(255,0,0)
    tool('mousemove','--window',window,'650','550');tool('click','1');time.sleep(.2)
    tool('key','ctrl+BackSpace');time.sleep(.2);tool('key','Delete');time.sleep(.2)
    tool('key','ctrl+a');time.sleep(.2)
    assert snapshot('rust-cancel-retained.png')==(255,0,0),'Delete removed an object after cancellation'
    tool('key','ctrl+z');time.sleep(.2)
    assert snapshot('rust-cancel-undo.png')==background,'Cancellation or Delete added an undo entry'
    tool('key','ctrl+shift+z');time.sleep(.2)
    assert snapshot('rust-cancel-redo.png')==(255,0,0)
    tool('key','Return');out,err=process.communicate(timeout=10)
    assert process.returncode==0,err.decode()
    assert Image.open(io.BytesIO(out)).convert('RGB').getpixel((650,550))==(255,0,0)
    print('PASS: cancel deselects the annotation; Delete is harmless; undo/redo and export retain the original drawing')
finally:
    if process.poll() is None:process.kill();process.wait()

'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=75)
    finally:
        display.terminate();display.wait(timeout=5)
