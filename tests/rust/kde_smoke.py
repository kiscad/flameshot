#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Private headless KWin/portal smoke. Usage: kde_smoke.py BINARY INPUT_HELPER [--clipboard]
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
        session=subprocess.Popen(['dbus-run-session','--config-file',str(bus),'--',sys.executable,__file__,str(binary),str(helper),*(['--clipboard'] if '--clipboard' in sys.argv else []),'--inside',str(root)],env=env,start_new_session=True)
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
    if '--clipboard' in sys.argv:
        daemon=start('flameshot-daemon',[str(binary)])
        wait_for(lambda:owned('org.flameshot.Flameshot'))
        def copy_capture(name,args):
            capture=start(name,[str(binary),*args],stdout=subprocess.PIPE)
            deadline=time.monotonic()+20
            while True:
                try:
                    out,_=capture.communicate(timeout=.4)
                    break
                except subprocess.TimeoutExpired:
                    assert time.monotonic()<deadline,'Clipboard capture timed out'
                    key(28)
            assert capture.returncode==0 and not out,(capture.returncode,out[:100])
        def read_clipboard(mime):
            destination=root/'clipboard-received'
            destination.unlink(missing_ok=True)
            code="""import sys
from pathlib import Path
from PyQt5.QtWidgets import QApplication,QWidget
from PyQt5.QtCore import QTimer
app=QApplication([])
assert app.platformName()=='wayland',app.platformName()
w=QWidget();w.setWindowTitle('Private Wayland clipboard receiver');w.resize(240,120);w.show()
clipboard=app.clipboard()
def read():
    data=clipboard.mimeData()
    if data is None:return
    if sys.argv[1]=='text':
        if not data.hasText():return
        payload=data.text().encode('utf-8')
    else:
        if not data.hasFormat(sys.argv[1]):return
        payload=bytes(data.data(sys.argv[1]))
    if payload:
        Path(sys.argv[2]).write_bytes(payload)
        app.quit()
timer=QTimer();timer.timeout.connect(read);timer.start(100)
QTimer.singleShot(8000,lambda:app.exit(2))
sys.exit(app.exec_())
"""
            receiver=start('clipboard-receiver',['/usr/bin/python3','-c',code,mime,str(destination)])
            assert receiver.wait(timeout=12)==0,'Native Qt receiver did not obtain clipboard data'
            return destination.read_bytes()
        for name,args,size,mime,tolerance in [
            ('clipboard-full',['full','--clipboard'],(1024,768),'image/png',0),
            ('clipboard-editor',['gui','--region','80x60+500+450','--clipboard'],(80,60),'image/png',0),
            ('clipboard-jpeg',['full','--clipboard'],(1024,768),'image/jpeg',2),
        ]:
            if mime=='image/jpeg':
                with (root/'config/flameshot/flameshot.ini').open('a') as config:config.write('useJpgForClipboard=true\n')
            copy_capture(name,args)
            payload=read_clipboard(mime)
            decoded=Image.open(io.BytesIO(payload))
            assert decoded.format==('JPEG' if mime=='image/jpeg' else 'PNG'),decoded.format
            image=decoded.convert('RGB')
            assert image.size==size,image.size
            assert all(abs(a-b)<=tolerance for a,b in zip(image.getpixel((20,20)),(32,64,96))),image.getpixel((20,20))
            print('PASS: KWin native Wayland receiver reads',mime,size,'after capture process exits',flush=True)
        text='KDE 剪贴板 — Unicode 🦀'
        result=subprocess.run(['gdbus','call','--session','--dest','org.flameshot.Flameshot','--object-path','/','--method','org.flameshot.Flameshot.attachTextToClipboard',text,''],env=env,capture_output=True,timeout=10)
        assert result.returncode==0,result.stderr
        assert read_clipboard('text').decode('utf-8')==text
        print('PASS: KWin native Wayland clipboard preserves Unicode text',flush=True)
        with (root/'config/flameshot/flameshot.ini').open('a') as config:config.write('autoCloseIdleDaemon=true\n')
        time.sleep(1.5)
        assert daemon.poll() is None,'Daemon discarded clipboard ownership while idle'
        replaced=root/'clipboard-replaced'
        writer_code="""import sys
from pathlib import Path
from PyQt5.QtWidgets import QApplication,QWidget
app=QApplication([])
assert app.platformName()=='wayland'
class Writer(QWidget):
    def keyPressEvent(self,event):
        app.clipboard().setText('Replacement from native Wayland receiver')
        Path(sys.argv[1]).touch()
w=Writer();w.resize(240,120);w.show()
app.exec_()
"""
        start('clipboard-writer',['/usr/bin/python3','-c',writer_code,str(replaced)])
        time.sleep(.5)
        key(57) # A real private-seat key event supplies a selection serial.
        wait_for(replaced.exists)
        assert daemon.wait(timeout=8)==0,'Idle daemon did not exit after ownership transfer'
        assert not owned('org.flameshot.Flameshot')
        assert read_clipboard('text').decode('utf-8')=='Replacement from native Wayland receiver'
        print('PASS: KWin clipboard replacement releases the Rust owner and allows idle daemon exit',flush=True)


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
