#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check runtime configuration fallback and live notices on private Xvfb.
Usage: config_fallback_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-config-fallback-') as directory:
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
assert 'updates' not in subprocess.check_output([binary,'--help'],text=True), 'Build without default features: fallback must not start real update requests in this fixture'
config=Path(os.environ['XDG_CONFIG_HOME'])/'flameshot/flameshot.ini';config.parent.mkdir(exist_ok=True)
valid='[General]\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\nshowHelp=false\n'
invalid=valid.replace('showHelp=false','showHelp=broken')
config.write_text(invalid)
probe=config.parent.parent/'probe';(probe/'flameshot').mkdir(parents=True)
(probe/'flameshot/flameshot.ini').write_text(valid)
def tool(*args):
    result=subprocess.run([xdotool,*args],capture_output=True,timeout=8)
    assert result.returncode==0,result.stderr
    return result.stdout.decode()
def snapshot(name):
    result=subprocess.run([binary,'full','--raw'],env=dict(os.environ,XDG_CONFIG_HOME=str(probe)),capture_output=True,timeout=10)
    assert result.returncode==0,result.stderr
    Path(artifacts,name).write_bytes(result.stdout)
    return Image.open(io.BytesIO(result.stdout)).convert('RGBA')
baseline=snapshot('rust-config-fallback-baseline.png')
process=subprocess.Popen([binary,'gui','--region','1024x768+0+0','--raw'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
try:
    deadline=time.monotonic()+15
    while True:
        result=subprocess.run([xdotool,'search','--onlyvisible','--name','^Flameshot$'],capture_output=True)
        if result.returncode==0:break
        assert process.poll() is None,process.communicate()
        assert time.monotonic()<deadline,'Missing editor'
        time.sleep(.1)
    window=result.stdout.decode().splitlines()[-1]
    tool('windowsize',window,'1024','768');tool('windowfocus','--sync',window);time.sleep(1.3)
    crop=(0,650,1024,740)  # Exclude the independently timed selection-geometry label.
    failed=snapshot('rust-config-fallback-error.png').crop(crop)
    assert ImageChops.difference(failed.convert('RGB'),baseline.crop(crop).convert('RGB')).getbbox(),'Missing configuration-error notice'
    assert config.read_text()==invalid
    tool('mousemove','--window',window,'900','730');time.sleep(.2)
    hidden=snapshot('rust-config-fallback-hover.png').crop(crop)
    assert not any(min(pixel[:3])>180 for pixel in hidden.getdata()),'Notice did not hide under pointer'
    tool('mousemove','--window',window,'500','400');time.sleep(.2)
    config.write_text(valid);time.sleep(1.3)
    resolved=snapshot('rust-config-fallback-resolved.png').crop(crop)
    assert ImageChops.difference(failed.convert('RGB'),resolved.convert('RGB')).getbbox(),'Error notice did not change after repair'
    config.write_text(invalid);time.sleep(1.3)
    again=snapshot('rust-config-fallback-again.png').crop(crop)
    assert again.tobytes()==failed.tobytes(),'Error notice did not return'
    config.write_text(valid);time.sleep(1.3)
    tool('windowfocus','--sync',window);tool('key','Return')
    out,err=process.communicate(timeout=15)
    assert process.returncode==0,err.decode()
    exported=Image.open(io.BytesIO(out)).convert('RGBA')
    assert exported.tobytes()==baseline.tobytes(),'Configuration notice leaked into screenshot export'
    messages=err.decode()
    assert messages.count('The configuration contains an error.')==2,messages
    assert messages.count('You have successfully resolved the configuration error.')==2,messages
    assert config.read_text()==valid
    print('PASS: invalid configuration captures with defaults; transition-only diagnostics, live notices and notice-free export')
finally:
    if process.poll() is None:process.kill();process.wait()

'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=75)
    finally:
        display.terminate();display.wait(timeout=5)
