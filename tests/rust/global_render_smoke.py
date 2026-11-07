#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check live global rendering settings without changing annotation history.
Usage: global_render_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-global-render-') as directory:
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
    tool('key','a');time.sleep(.2)
    tool('mousemove','--window',window,'550','500');tool('mousedown','1');tool('mousemove','--window',window,'700','580');time.sleep(.2);tool('mouseup','1');time.sleep(.3)
    tool('mousemove','--window',window,'550','610');tool('mousedown','1');tool('mousemove','--window',window,'700','610');time.sleep(.2);tool('mouseup','1');time.sleep(.3)
    arrows_only=snapshot('rust-global-arrows-only.png')
    tool('key','F10');click(800,520);tool('key','a');time.sleep(.3)
    original=snapshot('rust-global-original.png')
    def change(**values):
        import re
        source=config.read_text()
        for key,value in values.items():
            source=re.sub(r'(?m)^'+key+r'=.*$',key+'='+value,source)
        staged=config.with_suffix('.new');staged.write_text(source);staged.replace(config)
    def refreshed(name,expected=None):
        deadline=time.monotonic()+5
        while True:
            time.sleep(.3);pixels=snapshot(name)
            if expected is None and pixels.tobytes()!=original.tobytes():return pixels
            if expected is not None and pixels.tobytes()==expected.tobytes():return pixels
            assert time.monotonic()<deadline,'Global rendering preferences did not refresh'
    change(reverseArrow='true',drawCircleCounterOutline='true')
    updated=refreshed('rust-global-updated.png')
    assert updated.crop((0,0,230,140)).tobytes()!=original.crop((0,0,230,140)).tobytes()
    assert updated.crop((0,145,230,196)).tobytes()!=original.crop((0,145,230,196)).tobytes()
    assert updated.crop((270,30,340,115)).tobytes()!=original.crop((270,30,340,115)).tobytes()
    tool('key','ctrl+z');time.sleep(.3)
    undone=snapshot('rust-global-undo.png')
    assert undone.crop((0,0,230,196)).tobytes()==updated.crop((0,0,230,196)).tobytes(),'Undo reverted global preferences'
    assert undone.crop((270,30,340,115)).tobytes()==arrows_only.crop((270,30,340,115)).tobytes(),'Undo did not remove the last annotation'
    tool('key','ctrl+shift+z');time.sleep(.3)
    assert snapshot('rust-global-redo.png').tobytes()==updated.tobytes()
    change(reverseArrow='false',drawCircleCounterOutline='false')
    refreshed('rust-global-restored.png',original)
    tool('key','t');click(800,600);tool('type','--clearmodifiers','X');time.sleep(.3)
    snapshot('rust-global-text-before.png')
    change(reverseArrow='true')
    time.sleep(1.3)
    draft=snapshot('rust-global-text-after.png')
    assert draft.crop((0,0,230,196)).tobytes()==updated.crop((0,0,230,196)).tobytes(),'Text draft cached stale arrow rendering'
    click(110,240) # Cancel text draft.
    change(reverseArrow='false')
    refreshed('rust-global-text-cancel.png',original)
    tool('key','Return');out,err=process.communicate(timeout=10)
    assert process.returncode==0,err.decode()
    exported=Image.open(io.BytesIO(out)).convert('RGB')
    difference=ImageChops.difference(exported.crop((2,2,398,198)),original)
    assert exported.size==(400,200) and all(max(pixel)<=1 for pixel in difference.getdata())
    Path(artifacts,'rust-global-export.png').write_bytes(out)
    print('PASS: live arrow/counter rendering, unaffected undo history, text preview invalidation and export')

finally:
    if process.poll() is None:process.kill();process.wait()
'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=75)
    finally:
        display.terminate();display.wait(timeout=5)
