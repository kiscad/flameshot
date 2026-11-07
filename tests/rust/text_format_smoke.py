#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check text formatting preview, apply, undo and export on private Xvfb.
Usage: text_format_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-text-format-') as directory:
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
from PIL import Image,ImageChops
binary,xdotool,artifacts=sys.argv[1:]
config=Path(os.environ['XDG_CONFIG_HOME'])/'flameshot/flameshot.ini'
config.parent.mkdir(exist_ok=True)
config.write_text('[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\ndrawFontSize=24\n')
def tool(*args):
    result=subprocess.run([xdotool,*args],capture_output=True,timeout=5)
    assert result.returncode==0,result.stderr
    return result.stdout.decode()
def snapshot(name):
    captured=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
    assert captured.returncode==0,captured.stderr
    Path(artifacts,name).write_bytes(captured.stdout)
    return Image.open(io.BytesIO(captured.stdout)).convert('RGB').crop((502,452,898,648))
process=subprocess.Popen([binary,'gui','--region','400x200+500+450','--raw'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
try:
    deadline=time.monotonic()+15
    while True:
        found=subprocess.run([xdotool,'search','--onlyvisible','--name','^Flameshot$'],capture_output=True)
        if found.returncode==0:break
        assert time.monotonic()<deadline
        time.sleep(.1)
    window=found.stdout.decode().splitlines()[-1]
    tool('windowsize',window,'1024','768');tool('windowfocus','--sync',window);time.sleep(.5)
    tool('key','t');tool('mousemove','--window',window,'550','500');tool('click','1');time.sleep(.4)
    tool('type','--clearmodifiers','MMMM');tool('key','Return');tool('type','--clearmodifiers','I');time.sleep(.3)
    def click(x,y):
        tool('windowfocus','--sync',window)
        tool('mousemove','--window',window,str(x),str(y));tool('click','1');time.sleep(.3)
    original=snapshot('rust-text-format-dialog.png')
    previous=original
    for label,x in [('bold',34),('italic',87),('underline',140),('strikeout',223)]:
        click(x,176)
        current=snapshot('rust-text-format-'+label+'.png')
        assert current.tobytes()!=previous.tobytes(),label+' did not preview'
        previous=current
    def short_line_start(pixels):
        return min(x for y in range(90,140) for x in range(pixels.width) if pixels.getpixel((x,y))[0]>180 and max(pixels.getpixel((x,y))[1:])<80)
    left=short_line_start(previous)
    click(150,198);center=short_line_start(snapshot('rust-text-format-center.png'))
    click(243,198);styled=snapshot('rust-text-format-right.png');right=short_line_start(styled)
    assert left<center<right,(left,center,right)
    click(60,198);assert snapshot('rust-text-format-left.png').tobytes()==previous.tobytes()
    click(243,198)
    click(50,240) # Apply.
    assert snapshot('rust-text-format-applied.png').tobytes()==styled.tobytes()
    tool('mousemove','--window',window,'560','520');tool('click','--repeat','2','--delay','80','1');time.sleep(.4)
    click(34,176) # Turn bold off in the existing annotation draft.
    assert snapshot('rust-text-format-draft.png').tobytes()!=styled.tobytes()
    click(110,240) # Cancel.
    assert snapshot('rust-text-format-cancelled.png').tobytes()==styled.tobytes()
    tool('mousemove','--window',window,'560','520');tool('click','--repeat','2','--delay','80','1');time.sleep(.4)
    click(34,176);click(50,240)
    edited=snapshot('rust-text-format-edited.png')
    assert edited.tobytes()!=styled.tobytes()
    tool('key','ctrl+z');time.sleep(.3)
    assert snapshot('rust-text-format-undo.png').tobytes()==styled.tobytes()
    tool('key','ctrl+shift+z');time.sleep(.3)
    assert snapshot('rust-text-format-redo.png').tobytes()==edited.tobytes()
    tool('key','Return')
    out,err=process.communicate(timeout=10)
    assert process.returncode==0,err.decode()
    exported=Image.open(io.BytesIO(out)).convert('RGB')
    assert exported.size==(400,200)
    Path(artifacts,'rust-text-formatted-export.png').write_bytes(out)
    difference=ImageChops.difference(exported.crop((2,2,398,198)),edited)
    assert all(max(pixel)<=1 for pixel in difference.getdata()),'Export differs beyond display quantization'
    print('PASS: all text format controls, multiline alignment, cancel, undo/redo and preview/export pixels within display quantization')
finally:
    if process.poll() is None:process.kill();process.wait()
'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=75)
    finally:
        display.terminate();display.wait(timeout=5)
