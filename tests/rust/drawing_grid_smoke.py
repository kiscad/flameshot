#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check drawing grid snapping and exclusion from exports.
Usage: drawing_grid_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-drawing-grid-') as directory:
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
config.write_text('[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\nshowSelectionGeometry=0\nreverseArrow=false\ndrawCircleCounterOutline=false\nbuttons='+variant+'\n[Shortcuts]\nTYPE_CIRCLECOUNT=F10\n')
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
    def drag(x1,y1,x2,y2):
        tool('mousemove','--window',window,str(x1),str(y1));tool('mousedown','1')
        tool('mousemove','--window',window,str(x2),str(y2));time.sleep(.2);tool('mouseup','1');time.sleep(.3)
    blank=snapshot('rust-grid-blank.png')
    click(210,85)
    click(34,300)
    grid=snapshot('rust-grid-enabled.png')
    assert grid.tobytes()!=blank.tobytes(),'Grid was not displayed'
    assert grid.getpixel((8,8))!=blank.getpixel((8,8)),'Grid is not aligned at global (510,460)'
    tool('key','d');time.sleep(.2)
    drag(553,503,697,577)
    click(34,300)
    snapped=snapshot('rust-grid-snapped.png')
    tool('key','ctrl+z');time.sleep(.3)
    assert snapshot('rust-grid-undo.png').tobytes()==blank.tobytes()
    drag(550,500,700,580)
    assert snapshot('rust-grid-reference.png').tobytes()==snapped.tobytes(),'Snapped line differs from exact grid endpoints'
    tool('key','ctrl+z');time.sleep(.3)
    click(34,300)
    tool('mousemove','--window',window,'140','300');tool('click','--repeat','2','--delay','100','1');time.sleep(.2)
    tool('key','ctrl+a');tool('type','20');tool('key','Return');time.sleep(.3)
    click(400,400) # Defocus the numeric field without drawing in the selection.
    drag(553,503,697,577)
    click(34,300)
    wider=snapshot('rust-grid-spacing20.png')
    assert wider.tobytes()!=snapped.tobytes(),'Grid spacing change had no effect'
    tool('key','ctrl+z');time.sleep(.3)
    drag(560,500,700,580)
    assert snapshot('rust-grid-reference20.png').tobytes()==wider.tobytes()
    click(34,300)
    click(200,37) # Moving existing objects must not snap.
    drag(630,540,637,543)
    click(850,630)
    click(34,300)
    moved=snapshot('rust-grid-moved.png')
    tool('key','ctrl+z');time.sleep(.3)
    assert snapshot('rust-grid-move-undo.png').tobytes()==wider.tobytes()
    tool('key','ctrl+z');time.sleep(.3)
    tool('key','d');drag(567,503,707,583)
    assert snapshot('rust-grid-move-reference.png').tobytes()==moved.tobytes(),'Existing object movement snapped to grid'
    wider=moved
    click(34,300)
    assert snapshot('rust-grid-export-preview.png').tobytes()!=wider.tobytes()
    tool('key','t');click(803,603);tool('type','X');time.sleep(.3)
    draft=snapshot('rust-grid-text-draft.png')
    assert draft.getpixel((18,8))!=wider.getpixel((18,8)),'Text draft hid the grid'
    click(110,240)
    tool('key','Return');out,err=process.communicate(timeout=10)
    assert process.returncode==0,err.decode()
    exported=Image.open(io.BytesIO(out)).convert('RGB')
    difference=ImageChops.difference(exported.crop((2,2,398,198)),wider)
    assert exported.size==(400,200) and all(max(pixel)<=1 for pixel in difference.getdata()),'Grid leaked into export'
    Path(artifacts,'rust-grid-export.png').write_bytes(out)
    print('PASS: grid overlay, default/custom snapping, undo, exact-endpoint rendering and grid-free export')

finally:
    if process.poll() is None:process.kill();process.wait()
'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=75)
    finally:
        display.terminate();display.wait(timeout=5)
