#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check manual terminal launch on private Xvfb with a synthetic terminal/receiver.
Usage: terminal_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-terminal-') as directory:
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
import subprocess,sys,time,os,json
from pathlib import Path
binary,xdotool,artifacts=sys.argv[1:]
root=Path(os.environ['XDG_CONFIG_HOME'])
apps=root/'applications';apps.mkdir()
bin_dir=root/'bin';bin_dir.mkdir()
os.environ['XDG_DATA_DIRS']=str(root/'empty-data')
os.environ['PATH']=str(bin_dir)+':'+os.environ['PATH']
os.environ['TMPDIR']=str(root)
receipt=root/'receipt.json';terminal_receipt=root/'terminal.txt'
receiver=root/'receiver.py'
receiver.write_text('import json,sys\nfrom pathlib import Path\nfrom PIL import Image\nPath('+repr(str(receipt))+').write_text(json.dumps([sys.argv[1:],list(Image.open(sys.argv[1]).size)]))\n')
desktop=apps/'receiver.desktop'
desktop.write_text('[Desktop Entry]\nType=Application\nName=Test Receiver\nExec=/usr/bin/python3 "'+str(receiver)+'" %f %k\nTerminal=false\nMimeType=image/png;\n')
terminal=bin_dir/'gnome-terminal'
terminal.write_text('#!/usr/bin/python3\nimport os,sys\nfrom pathlib import Path\nPath('+repr(str(terminal_receipt))+').write_text("used")\nargs=sys.argv[1:]\nassert args[0]=="--",args\nos.execv(args[1],args[1:])\n')
terminal.chmod(0o755)
config=root/'flameshot/flameshot.ini';config.parent.mkdir(exist_ok=True)
config.write_text('[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\nuiLanguage=en\n')
def tool(*args):
    result=subprocess.run([xdotool,*map(str,args)],capture_output=True,timeout=5)
    assert result.returncode==0,(args,result.stderr)
    return result.stdout.decode()
process=subprocess.Popen([binary,'gui','--region','80x60+500+450','--raw'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
try:
    deadline=time.monotonic()+15
    while True:
        found=subprocess.run([xdotool,'search','--onlyvisible','--name','^Flameshot$'],capture_output=True)
        if found.returncode==0:break
        assert time.monotonic()<deadline
        time.sleep(.1)
    window=found.stdout.decode().splitlines()[-1]
    tool('windowfocus','--sync',window);time.sleep(.4)
    tool('key','ctrl+o');time.sleep(.7)
    captured=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
    assert captured.returncode==0,captured.stderr
    Path(artifacts,'rust-terminal-chooser.png').write_bytes(captured.stdout)
    tool('mousemove','--window',window,190,324);tool('click','1')
    tool('mousemove','--window',window,230,302);tool('click','1')
    out,err=process.communicate(timeout=15)
    assert process.returncode==0,err.decode()
    deadline=time.monotonic()+5
    while not receipt.exists():
        assert time.monotonic()<deadline,'Receiver did not run'
        time.sleep(.1)
    args,size=json.loads(receipt.read_text())
    assert size==[80,60],size
    assert args[1]==str(desktop),args
    assert terminal_receipt.read_text()=='used'
    Path(args[0]).unlink()
    print('PASS: native terminal checkbox routes selected 80x60 PNG through GIO terminal dispatch')
finally:
    if process.poll() is None:process.kill();process.wait()
'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=75)
    finally:
        display.terminate();display.wait(timeout=5)
