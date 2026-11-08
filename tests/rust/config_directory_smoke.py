#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check save-directory form validation on private Xvfb.
Usage: config_directory_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-config-directory-') as directory:
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
import subprocess,sys,time,os,configparser
from pathlib import Path
binary,xdotool,artifacts=sys.argv[1:]
assert 'updates' in subprocess.check_output([binary,'--help'],text=True),'This fixture uses the all-feature form layout'
config=Path(os.environ['XDG_CONFIG_HOME'])/'flameshot/flameshot.ini';config.parent.mkdir(exist_ok=True)
folder=config.parent/'saved images';folder.mkdir()
ordinary=config.parent/'ordinary file';ordinary.write_text('data')
original=f'; retained\r\n[General]\r\ncheckForUpdates=false\r\nsavePath={folder}\r\n[External]\r\nsavePath=opaque\r\n[General]\r\nsavePath={folder}\r\n'.encode()
config.write_bytes(original)
def tool(*args):
    result=subprocess.run([xdotool,*args],capture_output=True,timeout=8)
    assert result.returncode==0,result.stderr
    return result.stdout.decode()
process=subprocess.Popen([binary,'config'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
try:
    deadline=time.monotonic()+15
    while True:
        result=subprocess.run([xdotool,'search','--onlyvisible','--name','^Flameshot Configuration$'],capture_output=True)
        if result.returncode==0:break
        assert process.poll() is None,process.communicate()
        assert time.monotonic()<deadline,'Missing configuration window'
        time.sleep(.1)
    window=result.stdout.decode().splitlines()[-1]
    tool('windowmove',window,'0','0');tool('windowfocus','--sync',window);time.sleep(.4)
    def click(x,y):
        tool('mousemove','--window',window,str(x),str(y));tool('click','1');time.sleep(.2)
    def scroll(button):
        tool('mousemove','--window',window,'200','300');tool('click','--repeat','60','--delay','10',button);time.sleep(.3)
    def edit(value):
        scroll('4');click(100,107);tool('key','ctrl+a')
        if value:tool('type','--clearmodifiers','--delay','1',value)
        else:tool('key','BackSpace')
        scroll('5')
        capture=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
        assert capture.returncode==0,capture.stderr
        Path(artifacts,'rust-config-directory-before.png').write_bytes(capture.stdout)
        click(45,393)
        capture=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
        assert capture.returncode==0,capture.stderr
        Path(artifacts,'rust-config-directory-after.png').write_bytes(capture.stdout)
    for bad in [str(folder/'missing'),str(ordinary)]:
        edit(bad)
        assert config.read_bytes()==original,'Invalid directory changed the file'
    edit('')
    expected=original.replace(f'savePath={folder}\r\n'.encode(),b'')
    assert config.read_bytes()==expected,config.read_bytes()
    checked=subprocess.run([binary,'config','--check'],capture_output=True)
    assert checked.returncode==0,checked.stderr
    edit(str(folder))
    saved=configparser.ConfigParser(interpolation=None,strict=False);saved.read(config)
    assert saved.get('General','savePath').strip(chr(34))==str(folder)
    assert saved.get('External','savePath')=='opaque'
    assert subprocess.run([binary,'config','--check'],capture_output=True).returncode==0
    tool('key','Escape');out,err=process.communicate(timeout=10)
    assert process.returncode==0,err.decode()
    print('PASS: native form rejects missing/file paths, clears duplicate overrides, restores a valid directory and preserves external data')
finally:
    if process.poll() is None:process.kill();process.wait()

'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=75)
    finally:
        display.terminate();display.wait(timeout=5)
