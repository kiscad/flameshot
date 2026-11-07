#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Verify real Fcitx Pinyin input on private Xvfb and D-Bus.
Usage: ime_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-ime-') as directory:
    env=dict(os.environ,XDG_RUNTIME_DIR=directory,XDG_CONFIG_HOME=directory,XDG_CACHE_HOME=directory,
             XDG_DATA_HOME=directory,XDG_SESSION_TYPE='x11',LC_ALL='C.UTF-8',WINIT_X11_SCALE_FACTOR='1',XMODIFIERS='@im=fcitx',
             LD_LIBRARY_PATH=str(root/'usr/lib/x86_64-linux-gnu')+':'+os.environ.get('LD_LIBRARY_PATH',''))
    env.pop('WAYLAND_DISPLAY',None)
    config=Path(directory)/'bus.conf'
    config.write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
    display=subprocess.Popen([str(root/'usr/bin/Xvfb'),'-displayfd','1','-screen','0','1024x768x24','-nolisten','tcp','-noreset'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env)
    try:
        number=display.stdout.readline().decode().strip()
        assert number,'Xvfb did not start'
        env['DISPLAY']=':'+number
        driver=r"""
import subprocess,sys,time,os,io
from pathlib import Path
from PIL import Image
binary,xdotool,artifacts=sys.argv[1:]
config=Path(os.environ['XDG_CONFIG_HOME'])
(config/'fcitx5').mkdir()
(config/'fcitx5/profile').write_text('[Groups/0]\nName=Default\nDefault Layout=us\nDefaultIM=pinyin\n[Groups/0/Items/0]\nName=keyboard-us\nLayout=\n[Groups/0/Items/1]\nName=pinyin\nLayout=\n[GroupOrder]\n0=Default\n')
(config/'flameshot').mkdir()
(config/'flameshot/flameshot.ini').write_text('[General]\ncheckForUpdates=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\ndrawFontSize=24\n')
def tool(*args):return subprocess.run([xdotool,*args],capture_output=True,timeout=5)
def remote(*args):return subprocess.run(['fcitx5-remote',*args],capture_output=True,timeout=5)
fcitx=subprocess.Popen(['fcitx5','-D','--disable=cloudpinyin','--verbose=default=2'],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
editor=None
passed=False
try:
    deadline=time.monotonic()+10
    while remote('--check').returncode:
        assert fcitx.poll() is None and time.monotonic()<deadline,'Fcitx did not start'
        time.sleep(.1)
    time.sleep(1)
    editor=subprocess.Popen([binary,'gui','--raw','--region','400x200+500+450'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    deadline=time.monotonic()+10
    while True:
        found=tool('search','--onlyvisible','--name','^Flameshot$')
        if found.returncode==0:break
        assert editor.poll() is None and time.monotonic()<deadline,'Editor not visible'
        time.sleep(.1)
    window=found.stdout.decode().splitlines()[-1]
    # Xvfb has no window manager to honor fullscreen requests.
    assert tool('windowsize','--sync',window,'1024','768').returncode==0
    tool('windowfocus','--sync',window);time.sleep(.4)
    tool('key','t');tool('mousemove','--window',window,'550','500');tool('click','1');time.sleep(.4)
    assert remote('-s','pinyin').returncode==0
    remote('-c');tool('key','ctrl+space');time.sleep(.4)
    assert remote().stdout.strip()==b'2'
    assert remote('-n').stdout.strip()==b'pinyin'
    tool('type','--clearmodifiers','--delay','100','nihao');tool('key','space');time.sleep(3)
    debug=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
    Path(artifacts,'rust-ime-debug.png').write_bytes(debug.stdout)
    remote('-c');time.sleep(.2)
    tool('key','--delay','100','ctrl+a');time.sleep(.3)
    tool('key','--delay','100','ctrl+c');time.sleep(.3)
    deadline=time.monotonic()+5
    while True:
        copied=subprocess.run([str(Path(xdotool).with_name('xclip')),'-selection','clipboard','-t','UTF8_STRING','-o'],capture_output=True,timeout=5)
        if copied.stdout.decode()=='你好':break
        assert time.monotonic()<deadline,(copied.stdout,copied.stderr)
        time.sleep(.1)
    tool('type','--clearmodifiers','x');time.sleep(.2)
    tool('key','--delay','100','ctrl+a');tool('key','--delay','100','ctrl+v');time.sleep(.3)
    tool('key','--delay','100','ctrl+a');tool('key','--delay','100','ctrl+c');time.sleep(.3)
    copied=subprocess.run([str(Path(xdotool).with_name('xclip')),'-selection','clipboard','-t','UTF8_STRING','-o'],capture_output=True,timeout=5)
    assert copied.stdout.decode()=='你好','Native text paste did not restore Chinese'
    tool('key','End');tool('key','ctrl+space');time.sleep(.3)
    tool('type','--clearmodifiers','--delay','100','zhong');time.sleep(.5)
    tool('key','Escape');time.sleep(.3)
    assert editor.poll() is None,'IME cancellation closed the editor'
    remote('-c');time.sleep(.2)
    tool('key','--delay','100','ctrl+a');tool('key','--delay','100','ctrl+c');time.sleep(.3)
    copied=subprocess.run([str(Path(xdotool).with_name('xclip')),'-selection','clipboard','-t','UTF8_STRING','-o'],capture_output=True,timeout=5)
    assert copied.stdout.decode()=='你好','IME cancellation changed committed text'
    remote('-c');tool('key','ctrl+Return');time.sleep(.4)
    assert editor.poll() is None
    tool('key','Return')
    out,err=editor.communicate(timeout=10)
    assert editor.returncode==0,err.decode()
    image=Image.open(io.BytesIO(out)).convert('RGB')
    assert image.size==(400,200) and image.getbbox() is not None
    bounds=image.getbbox()
    assert 0<bounds[0]<bounds[2]<400 and 0<bounds[1]<bounds[3]<200,bounds
    Path(artifacts,'rust-ime-pinyin.png').write_bytes(out)
    passed=True
    print('PASS: real Fcitx Pinyin commits Chinese through XIM and exports the text annotation')
finally:
    if editor is not None:
        if editor.poll() is None:editor.kill();editor.wait()
        out,err=editor.communicate()
        if not passed:print(err.decode(),file=sys.stderr)
    remote('-e')
    try:fcitx.wait(timeout=5)
    except subprocess.TimeoutExpired:fcitx.kill();fcitx.wait()
    if not passed:print(fcitx.stderr.read().decode(),file=sys.stderr)
    assert fcitx.returncode==0,'Private Fcitx did not exit cleanly'
"""
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=50)
    finally:
        display.terminate();display.wait(timeout=5)
