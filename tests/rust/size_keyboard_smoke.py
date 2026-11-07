#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check numeric tool-size entry and its interaction with shortcuts and editing.
Usage: size_keyboard_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-size-keyboard-') as directory:
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
config.write_text('[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\nshowSelectionGeometry=0\nreverseArrow=false\ndrawCircleCounterOutline=false\nbuttons='+variant+'\n[Shortcuts]\nTYPE_UNDO=7\nTYPE_ARROW=8,9\n')
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
    def value(name):
        import re
        match=re.search(r'(?m)^'+name+r'=(.*)$',config.read_text())
        return match.group(1).strip('"') if match else None
    def keys(*items):
        tool('key',*items);time.sleep(.25)
    keys('p','2','5')
    assert value('drawThickness')=='25',config.read_text()
    keys('x','9','9')
    assert value('drawThickness')=='50'
    keys('3')
    assert value('drawThickness')=='3','Overflow should reset decimal entry'
    keys('Num_Lock','x')
    # Send physical keypad events: xdotool's keysym helper toggles NumLock
    # around each KP_* key, unlike typing on a real keypad.
    import ctypes
    x11=ctypes.CDLL('libX11.so.6');xtst=ctypes.CDLL('libXtst.so.6')
    x11.XOpenDisplay.argtypes=[ctypes.c_char_p];x11.XOpenDisplay.restype=ctypes.c_void_p
    x11.XStringToKeysym.argtypes=[ctypes.c_char_p];x11.XStringToKeysym.restype=ctypes.c_ulong
    x11.XKeysymToKeycode.argtypes=[ctypes.c_void_p,ctypes.c_ulong];x11.XKeysymToKeycode.restype=ctypes.c_uint
    x11.XFlush.argtypes=[ctypes.c_void_p];x11.XCloseDisplay.argtypes=[ctypes.c_void_p]
    xtst.XTestFakeKeyEvent.argtypes=[ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_ulong]
    connection=x11.XOpenDisplay(None);assert connection
    try:
        for name in [b'KP_2',b'KP_5']:
            code=x11.XKeysymToKeycode(connection,x11.XStringToKeysym(name));assert code
            for pressed in [1,0]:
                assert xtst.XTestFakeKeyEvent(connection,code,pressed,0)
                x11.XFlush(connection);time.sleep(.1)
    finally:x11.XCloseDisplay(connection)
    time.sleep(.3)
    assert value('drawThickness')=='25','Keypad digits did not set size'
    blank=snapshot('rust-size-keyboard-blank.png')
    tool('mousemove','--window',window,'550','500');tool('mousedown','1');tool('mousemove','--window',window,'700','580');time.sleep(.2);tool('mouseup','1');time.sleep(.3)
    drawn=snapshot('rust-size-keyboard-drawn.png')
    assert drawn.tobytes()!=blank.tobytes()
    keys('7')
    assert value('drawThickness')=='25','Digit shortcut also changed size'
    assert snapshot('rust-size-keyboard-undo.png').tobytes()==blank.tobytes()
    keys('ctrl+shift+z')
    assert snapshot('rust-size-keyboard-redo.png').tobytes()==drawn.tobytes()
    keys('8')
    assert value('drawThickness')=='25','Shortcut prefix changed size'
    keys('9')
    assert value('drawThickness')=='25','Shortcut completion changed size'
    keys('t');click(800,600);tool('type','--clearmodifiers','123');time.sleep(.3)
    assert value('drawFontSize') is None,'Text entry changed font size'
    click(110,240)
    keys('p','4')
    assert value('drawThickness')=='4','Text editing did not reset numeric entry'
    click(200,37) # Move object.
    click(625,540)
    keys('x','1','2')
    assert value('drawThickness')=='12'
    click(850,630)
    changed=snapshot('rust-size-keyboard-selected.png')
    assert changed.tobytes()!=drawn.tobytes(),'Selected object did not resize'
    keys('7','7')
    assert snapshot('rust-size-keyboard-selected-undo.png').tobytes()==drawn.tobytes()
    keys('Return');out,err=process.communicate(timeout=10)
    assert process.returncode==0,err.decode()
    exported=Image.open(io.BytesIO(out)).convert('RGB')
    difference=ImageChops.difference(exported.crop((2,2,398,198)),drawn)
    assert exported.size==(400,200) and all(max(pixel)<=1 for pixel in difference.getdata())
    Path(artifacts,'rust-size-keyboard-export.png').write_bytes(out)
    print('PASS: decimal/keypad size entry, clamping, digit shortcuts, text isolation, selected resize and undo/export')

finally:
    if process.poll() is None:process.kill();process.wait()
'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=75)
    finally:
        display.terminate();display.wait(timeout=5)
