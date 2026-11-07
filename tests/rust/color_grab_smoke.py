#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check modal color grabbing, cancellation and selected-object recoloring.
Usage: color_grab_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-color-grab-') as directory:
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
from PIL import Image,ImageChops
binary,xdotool,artifacts=sys.argv[1:]
config=Path(os.environ['XDG_CONFIG_HOME'])/'flameshot/flameshot.ini';config.parent.mkdir(exist_ok=True)
data=struct.pack('>II',127,11)+b'QList<int>\0'+struct.pack('>I',0)
variant='@Variant('+''.join(chr(92)+'x'+format(byte,'02x') for byte in data)+')'
config.write_text('[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\nshowSelectionGeometry=0\nuserColors=#00ff00, #0000ff, #ff0000\nreverseArrow=false\ndrawCircleCounterOutline=false\nbuttons='+variant+'\n[Shortcuts]\nTYPE_CIRCLECOUNT=F10\n')
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
    def click(x,y):
        tool('windowfocus','--sync',window);tool('mousemove','--window',window,str(x),str(y));tool('click','1');time.sleep(.3)
    tool('windowsize',window,'1024','768');tool('windowfocus','--sync',window);time.sleep(.5)
    def keys(*args):
        tool('key',*args);time.sleep(.3)
    def drag(x1,y1,x2,y2):
        tool('mousemove','--window',window,str(x1),str(y1));tool('mousedown','1')
        tool('mousemove','--window',window,str(x2),str(y2));time.sleep(.2);tool('mouseup','1');time.sleep(.3)
    def saved_color():
        import re
        match=re.search(r'(?m)^drawColor=(.*)$',config.read_text())
        return match.group(1).strip('"').lower() if match else None
    def colors(name):
        pixels=snapshot(name)
        return pixels.getpixel((68,68)),pixels.getpixel((248,68))
    keys('r');drag(530,480,630,560)
    click(77,60);drag(700,480,800,560)
    assert colors('rust-grab-original.png')==((255,0,0),(0,0,255))
    click(200,37);click(570,520)
    before=config.read_text()
    tool('mousemove','--window',window,'750','520');keys('g')
    assert config.read_text()==before,'Entering color grab wrote settings'
    keys('Escape')
    assert process.poll() is None,'Escape closed the editor instead of cancelling color grab'
    assert config.read_text()==before
    assert colors('rust-grab-cancel.png')==((255,0,0),(0,0,255))
    keys('g');click(750,520)
    assert colors('rust-grab-confirm.png')==((0,0,255),(0,0,255)),'Picked color did not update selected object'
    keys('ctrl+z')
    assert colors('rust-grab-undo.png')==((255,0,0),(0,0,255))
    keys('ctrl+shift+z')
    assert colors('rust-grab-redo.png')==((0,0,255),(0,0,255))
    click(570,520);keys('space')
    snapshot('rust-grab-panel.png')
    click(75,365)
    tool('mousemove','--window',window,'850','600');time.sleep(.3)
    active=snapshot('rust-grab-button-active.png')
    full=Image.open(Path(artifacts,'rust-grab-button-active.png')).convert('RGB')
    assert full.getpixel((34,37))==(0,0,0),'Grab Color button did not enter picking mode'
    tool('click','3');time.sleep(.3)
    assert snapshot('rust-grab-magnifier.png').tobytes()!=active.tobytes(),'Right click did not show magnifier'
    keys('space')
    assert snapshot('rust-grab-magnifier-off.png').tobytes()==active.tobytes(),'Space did not hide magnifier'
    keys('Escape')
    assert colors('rust-grab-right-cancel.png')==((0,0,255),(0,0,255))
    keys('space','p')
    tool('mousemove','--window',window,'850','600');keys('g','Return')
    assert process.poll() is None,'Accepting a color with Return exported the capture'
    assert colors('rust-grab-default.png')==((0,0,255),(0,0,255)),'Drawing default pick recolored an existing object'
    assert saved_color()=='#000000ff'
    click(200,37);click(850,630)
    keys('r');click(40,60);drag(650,480,652,560)
    precise_source=snapshot('rust-grab-precision-source.png')
    assert precise_source.getpixel((149,68))==(0,255,0)
    assert precise_source.getpixel((177,68))==(0,0,0)
    click(77,60)
    before=config.read_text()
    tool('mousemove','--window',window,'649','520');keys('g')
    tool('mousedown','1');time.sleep(.3)
    tool('mousemove','--window',window,'679','520');time.sleep(.3)
    held=snapshot('rust-grab-precision-held.png')
    assert held.tobytes()!=precise_source.tobytes(),'Held-button pixel grid was not displayed'
    assert config.read_text()==before,'Holding a precision pick committed early'
    tool('mouseup','1');time.sleep(.3)
    assert saved_color()=='#00ff00ff','Zoomed cell did not select source pixel 651'
    assert snapshot('rust-grab-precision-release.png').tobytes()==precise_source.tobytes(),'Precision pick changed document pixels'
    before=config.read_text()
    tool('mousemove','--window',window,'649','520');keys('g')
    tool('mousedown','1');time.sleep(.2);tool('mousemove','--window',window,'679','520');time.sleep(.2)
    keys('Escape');tool('mouseup','1');time.sleep(.3)
    assert config.read_text()==before
    assert snapshot('rust-grab-precision-cancel.png').tobytes()==precise_source.tobytes(),'Cancelling a held pick drew an annotation'
    click(77,60)
    tool('mousemove','--window',window,'649','520');keys('g')
    tool('mousedown','1');time.sleep(.2);tool('mousemove','--window',window,'679','520');time.sleep(.2)
    keys('Return');tool('mouseup','1');time.sleep(.3)
    assert process.poll() is None and saved_color()=='#00ff00ff'
    assert snapshot('rust-grab-precision-enter.png').tobytes()==precise_source.tobytes(),'Enter confirmation leaked the held mouse release'
    before=config.read_text()
    tool('mousemove','--window',window,'649','520');keys('g')
    tool('mousedown','1');time.sleep(.2);tool('mousemove','--window',window,'850','600');time.sleep(.2)
    tool('mouseup','1');time.sleep(.3)
    assert config.read_text()==before,'Leaving the precision grid committed a different pixel'
    keys('Escape')
    tool('mousemove','--window',window,'1023','767');keys('g')
    tool('mousedown','1');time.sleep(.2);tool('mousemove','--window',window,'1008','752');time.sleep(.2)
    tool('mouseup','1');time.sleep(.3)
    assert saved_color()=='#000000ff','Edge-clipped precision pick failed'
    assert snapshot('rust-grab-precision-edge.png').tobytes()==precise_source.tobytes()
    expected=snapshot('rust-grab-export-preview.png')
    keys('Return');out,err=process.communicate(timeout=10)
    assert process.returncode==0,err.decode()
    exported=Image.open(io.BytesIO(out)).convert('RGB')
    difference=ImageChops.difference(exported.crop((2,2,398,198)),expected)
    assert exported.size==(400,200) and all(max(pixel)<=1 for pixel in difference.getdata())
    Path(artifacts,'rust-grab-export.png').write_bytes(out)
    print('PASS: color-grab preview/cancel, selected-object recolor, undo/redo, button activation, default-only pick, held pixel selection/cancel/edge handling and export')

finally:
    if process.poll() is None:process.kill();process.wait()
'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=75)
    finally:
        display.terminate();display.wait(timeout=5)
