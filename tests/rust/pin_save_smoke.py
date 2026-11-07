#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check pinned-image saving with the shared chooser.
Usage: pin_save_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-pin-save-') as directory:
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
base=Path(os.environ['XDG_CONFIG_HOME']);folder=base/'saved';folder.mkdir()
config=base/'flameshot/flameshot.ini';config.parent.mkdir(exist_ok=True)
config.write_text(f'[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\ndisabledTrayIcon=true\nuseX11LegacyScreenshot=true\nsavePath={folder}\nfilenamePattern=pin-save\nfutureKey=keep\n')
def tool(*args):
    r=subprocess.run([xdotool,*args],capture_output=True,timeout=5);assert r.returncode==0,r.stderr
    return r.stdout.decode()
def find(title):
    r=subprocess.run([xdotool,'search','--onlyvisible','--name','^'+title+'$'],capture_output=True)
    return r.stdout.decode().splitlines()[-1] if r.returncode==0 else None
def wait(predicate):
    deadline=time.monotonic()+10
    while True:
        value=predicate()
        if value:return value
        assert time.monotonic()<deadline,'Timed out waiting for pin/save window'
        time.sleep(.1)
source=Image.new('RGBA',(400,300),(255,0,0,255));source.paste((0,255,0,255),(200,0,400,300))
expected=source.transpose(Image.Transpose.ROTATE_270)
buffer=io.BytesIO();source.save(buffer,format='PNG')
wire=base/'pin.bin';wire.write_bytes(struct.pack('>I',1)+buffer.getvalue()+struct.pack('>iiiid',100,100,499,399,1.0))
with wire.open('rb') as stdin:
    process=subprocess.Popen([binary,'internal-pin'],stdin=stdin,stdout=subprocess.PIPE,stderr=subprocess.PIPE,cwd=folder)
try:
    pin=wait(lambda:find('flameshot-pin'));tool('windowfocus','--sync',pin);time.sleep(.4)
    def menu(index):
        tool('windowfocus','--sync',pin);tool('mousemove','--window',pin,'20','20');tool('click','3');time.sleep(.2)
        tool('key','--delay','150',*(['Down']*index),'Return');time.sleep(.3)
    menu(3)
    tool('mousemove','--window',pin,'150','200');tool('click','4');tool('key','5');time.sleep(.3)
    def open_save():
        menu(2);dialog=wait(lambda:find('Save Screenshot'));tool('windowfocus','--sync',dialog);time.sleep(.3);return dialog
    dialog=open_save();tool('key','Escape')
    wait(lambda:find('Save Screenshot') is None)
    assert process.poll() is None and not list(folder.iterdir()),'Cancel closed pin or wrote a file'
    time.sleep(.3)
    geometry=dict(line.split('=',1) for line in tool('getwindowgeometry','--shell',pin).splitlines() if '=' in line)
    frame=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10);assert frame.returncode==0,frame.stderr
    frame=Image.open(io.BytesIO(frame.stdout)).convert('RGB')
    x,y,w,h=(int(geometry[key]) for key in ['X','Y','WIDTH','HEIGHT'])
    assert abs(frame.getpixel((x+w//2,y+h//4))[0]-128)<=2,'Cancel did not restore visible pin pixels'
    assert abs(frame.getpixel((x+w//2,y+3*h//4))[1]-128)<=2,'Cancel changed pin opacity'

    dialog=open_save()
    def destination(path):
        tool('key','ctrl+a');tool('type','--clearmodifiers',str(path));tool('key','Return');time.sleep(.4)
    destination(base/'missing/failure.png')
    assert find('Save Screenshot') and process.poll() is None
    target=folder/'rotated.png';destination(target)
    wait(lambda:find('Save Screenshot') is None)
    assert Image.open(target).convert('RGBA').tobytes()==expected.tobytes()
    assert Image.open(target).size==expected.size and 'futureKey=keep' in config.read_text()
    target.write_bytes(b'preserve until confirmed')
    dialog=open_save();destination(target)
    assert target.read_bytes()==b'preserve until confirmed'
    captured=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10);assert captured.returncode==0,captured.stderr
    Path(artifacts,'rust-pin-save-confirm.png').write_bytes(captured.stdout)
    tool('key','Return');time.sleep(.3) # Default No.
    assert target.read_bytes()==b'preserve until confirmed' and find('Save Screenshot')
    tool('key','Return');time.sleep(.4) # Request save again.
    tool('key','--delay','150','shift+Tab','Return')
    wait(lambda:find('Save Screenshot') is None)
    assert Image.open(target).convert('RGBA').tobytes()==expected.tobytes()
    assert not (folder/'rotated_1.png').exists(),'Confirmed overwrite created a sibling'
    dialog=open_save();destination('relative.png')
    wait(lambda:find('Save Screenshot') is None)
    assert Image.open(folder/'relative.png').convert('RGBA').tobytes()==expected.tobytes()
    assert str(folder) in config.read_text(),'Relative save forgot the actual destination directory'
    config.write_text(config.read_text()+'copyPathAfterSave=true\n')
    dialog=open_save();final=folder/'copied-path.png';destination(final)
    wait(lambda:find('Save Screenshot') is None)
    tool('windowfocus','--sync',pin);tool('key','Escape')
    out,err=process.communicate(timeout=10);assert process.returncode==0,err.decode()
    xclip=str(Path(xdotool).with_name('xclip'))
    copied=subprocess.run([xclip,'-selection','clipboard','-t','UTF8_STRING','-o'],capture_output=True,timeout=5)
    assert copied.returncode==0 and copied.stdout.decode()==str(final),copied.stderr
    assert Image.open(final).convert('RGBA').tobytes()==expected.tobytes()
    Path(artifacts,'rust-pin-saved.png').write_bytes(final.read_bytes())
    config.write_text(f'[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\ndisabledTrayIcon=true\nsavePath={folder}\nsavePathFixed=true\nsaveAsFileExtension=bmp\nfilenamePattern=fixed-pin\n')
    with wire.open('rb') as stdin:
        process=subprocess.Popen([binary,'internal-pin'],stdin=stdin,stdout=subprocess.PIPE,stderr=subprocess.PIPE,cwd=folder)
    pin=wait(lambda:find('flameshot-pin'));tool('windowfocus','--sync',pin);time.sleep(.4)
    menu(3);menu(2)
    fixed=folder/'fixed-pin.bmp';wait(fixed.exists)
    assert find('Save Screenshot') is None and process.poll() is None
    assert Image.open(fixed).convert('RGBA').tobytes()==expected.tobytes()
    menu(2);wait(lambda:(folder/'fixed-pin_1.bmp').exists())
    assert Image.open(fixed).convert('RGBA').tobytes()==expected.tobytes()
    tool('key','Escape');out,err=process.communicate(timeout=10);assert process.returncode==0,err.decode()
    manual_folder=base/'manual-parent';manual_folder.mkdir()
    config.write_text(f'[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\nsavePath={folder}\n')
    process=subprocess.Popen([binary,'gui','--region','80x60+10+20','--raw'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,cwd=folder)
    editor=wait(lambda:find('Flameshot'));tool('windowsize','--sync',editor,'1024','768');tool('windowfocus','--sync',editor);time.sleep(.4)
    tool('key','ctrl+s');time.sleep(.3);destination(manual_folder/'editor.png')
    out,err=process.communicate(timeout=10);assert process.returncode==0,err.decode()
    assert Image.open(io.BytesIO(out)).size==(80,60)
    import re
    assert re.search(r'(?m)^savePath=(.*)$',config.read_text()).group(1).strip('"')==str(manual_folder),'Editor did not remember manual save directory'
    config.write_text('[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\nsavePathFixed=true\nfilenamePattern=editor-fixed\n')
    process=subprocess.Popen([binary,'gui','--region','80x60+10+20','--raw'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,cwd=folder,env=dict(os.environ,HOME=str(folder)))
    editor=wait(lambda:find('Flameshot'));tool('windowsize','--sync',editor,'1024','768');tool('windowfocus','--sync',editor);time.sleep(.4)
    tool('key','ctrl+s');out,err=process.communicate(timeout=10);assert process.returncode==0,err.decode()
    assert (folder/'editor-fixed.png').exists()
    assert re.search(r'(?m)^savePath=(.*)$',config.read_text()).group(1).strip('"')==str(folder),'Fixed editor save did not remember the default directory'
    print('PASS: pin save cancel, failure/retry, full rotated pixels, overwrite confirmation path clipboard after close and fixed-path BMP saves')
finally:
    if process.poll() is None:process.kill();process.wait()
'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=100)
    finally:
        display.terminate();display.wait(timeout=5)
