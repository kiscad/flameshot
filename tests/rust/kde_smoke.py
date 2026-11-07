#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Private headless KWin/portal smoke. Usage: kde_smoke.py BINARY INPUT_HELPER
Requires KWin 5, KDE portal, Qt Wayland, PyQt5, Pillow and private-session tools.
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import io
import signal
import shutil
from PIL import Image
binary,helper=map(lambda p:Path(p).resolve(),sys.argv[1:3])
if '--inside' not in sys.argv:
    with tempfile.TemporaryDirectory(prefix='flameshot-kde-') as directory:
        root=Path(directory)
        for name in ['runtime','config','data','cache','tmp','pictures']:(root/name).mkdir(mode=0o700)
        config=root/'config/flameshot';config.mkdir()
        (config/'flameshot.ini').write_text('[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\n')
        applications=root/'data/applications';applications.mkdir()
        (applications/'flameshot-kde-input.desktop').write_text(f'[Desktop Entry]\nType=Application\nName=Isolated test input\nExec={helper}\nX-KDE-Wayland-Interfaces=org_kde_kwin_fake_input\n')
        portals=root/'config/xdg-desktop-portal';portals.mkdir()
        (portals/'portals.conf').write_text('[preferred]\ndefault=kde\n')
        (root/'config/user-dirs.dirs').write_text(f'XDG_PICTURES_DIR="{root}/pictures"\n')
        bus=root/'bus.conf';bus.write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
        env=os.environ.copy()
        for key in ['DISPLAY','WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS','DBUS_SYSTEM_BUS_ADDRESS','SESSION_MANAGER']:env.pop(key,None)
        env.update(XDG_RUNTIME_DIR=str(root/'runtime'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),TMPDIR=str(root/'tmp'),XDG_CURRENT_DESKTOP='KDE',XDG_SESSION_TYPE='wayland',QT_QPA_PLATFORM='wayland',LIBGL_ALWAYS_SOFTWARE='1',KWIN_COMPOSE='O2')
        session=subprocess.Popen(['dbus-run-session','--config-file',str(bus),'--',sys.executable,__file__,str(binary),str(helper),'--inside',str(root)],env=env,start_new_session=True)
        try:
            status=session.wait(timeout=100)
        except subprocess.TimeoutExpired:
            os.killpg(session.pid,signal.SIGTERM)
            try:session.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(session.pid,signal.SIGKILL);session.wait()
            status=124
        if status:
            for log in root.glob('*.log'):print(log.name,log.read_text(errors='replace')[-6000:],file=sys.stderr)
        sys.exit(status)
root=Path(sys.argv[-1]);env=os.environ.copy();env['DBUS_SYSTEM_BUS_ADDRESS']=env['DBUS_SESSION_BUS_ADDRESS']
processes=[];logs=[]
def start(name,args,extra=None,**kwargs):
    log=(root/(name+'.log')).open('w');logs.append(log)
    process=subprocess.Popen(args,env=dict(env,**(extra or {})),stderr=log,stdout=kwargs.pop('stdout',log),**kwargs);processes.append(process);return process

def wait_for(predicate,timeout=15):
    deadline=time.monotonic()+timeout
    while time.monotonic()<deadline:
        if predicate():return
        time.sleep(.1)
    raise AssertionError('Timed out waiting for private KWin service')
def owned(name):
    result=subprocess.run(['gdbus','call','--session','--dest','org.freedesktop.DBus','--object-path','/org/freedesktop/DBus','--method','org.freedesktop.DBus.NameHasOwner',name],env=env,capture_output=True,timeout=3)
    return result.returncode==0 and b'true' in result.stdout
try:
    subprocess.run(['kbuildsycoca5','--noincremental'],env=env,capture_output=True,check=True,timeout=15)
    compositor=start('kwin',['kwin_wayland','--virtual','--width','1024','--height','768','--no-lockscreen','--no-global-shortcuts','--socket','flameshot-kde'],{'QT_QPA_PLATFORM':'offscreen','QT_LOGGING_RULES':'kwin_core.debug=true;KWIN_UTILS.debug=true'})
    wait_for(lambda:(root/'runtime/flameshot-kde').exists() or compositor.poll() is not None)
    assert compositor.poll() is None,'KWin exited'
    env['WAYLAND_DISPLAY']='flameshot-kde'
    pointer=start('input',[str(helper)],{'FLAMESHOT_TEST_KDE_INPUT':'1'},stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
    assert pointer.stdout.readline().strip()=='ready'
    def key(code):
        for state in [1,0]:
            pointer.stdin.write(f'key {code} {state}\n');pointer.stdin.flush();assert pointer.stdout.readline().strip()=='ok'
    start('background',['/usr/bin/python3','-c','from PyQt5.QtWidgets import QApplication,QWidget; app=QApplication([]); w=QWidget(); w.setStyleSheet("background:rgb(32,64,96)"); w.showFullScreen(); app.exec_()'])
    start('permission-store',['/usr/libexec/xdg-permission-store'])
    start('portal-kde',['/usr/lib/x86_64-linux-gnu/libexec/xdg-desktop-portal-kde'])
    wait_for(lambda:owned('org.freedesktop.impl.portal.desktop.kde'))
    start('portal',['/usr/libexec/xdg-desktop-portal'])
    wait_for(lambda:owned('org.freedesktop.portal.Desktop'))
    time.sleep(1)
    for command,args,size,accept in [('full',['full','--raw'],(1024,768),True),('screen',['screen','--number','0','--raw'],(1024,768),True),('editor',['gui','--region','80x60+500+450','--raw'],(80,60),True),('cancel',['gui','--region','80x60+500+450','--raw'],None,False)]:
        capture=start('capture-'+command,[str(binary),*args],stdout=subprocess.PIPE)
        deadline=time.monotonic()+20
        while True:
            try:
                data,_=capture.communicate(timeout=.4)
                break
            except subprocess.TimeoutExpired:
                assert time.monotonic()<deadline,'KDE screenshot request timed out'
                key(28 if accept else 1) # Private-session consent/accept or Escape.
        if not accept:
            assert capture.returncode==2 and not data,(capture.returncode,len(data))
            print('PASS: KWin cancellation returns 2 with empty stdout',flush=True)
            continue
        assert capture.returncode==0,(command,capture.returncode)
        image=Image.open(io.BytesIO(data)).convert('RGB')
        assert image.size==size,image.size
        assert image.getpixel((20,20))==(32,64,96),image.getpixel((20,20))
        if os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR'):
            artifacts=Path(os.environ['FLAMESHOT_TEST_ARTIFACT_DIR']);artifacts.mkdir(parents=True,exist_ok=True)
            (artifacts/('kde-'+command+'.png')).write_bytes(data)
        print('PASS: KWin/KDE real portal',command,size,'and background pixels',flush=True)
finally:
    for process in reversed(processes):
        if process.poll() is None:
            process.terminate()
            try:process.wait(timeout=4)
            except subprocess.TimeoutExpired:process.kill();process.wait(timeout=4)
    for log in logs:log.close()
    if os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR'):
        artifacts=Path(os.environ['FLAMESHOT_TEST_ARTIFACT_DIR']);artifacts.mkdir(parents=True,exist_ok=True)
        for log in root.glob('*.log'):shutil.copy(log,artifacts/log.name)
