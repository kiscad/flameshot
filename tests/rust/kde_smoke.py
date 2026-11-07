#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Private headless KWin/portal smoke. Usage: kde_smoke.py BINARY INPUT_HELPER [--clipboard] [--multi-monitor] [--fractional] [--hotplug] [--ime]
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
import json
from PIL import Image
binary,helper=map(lambda p:Path(p).resolve(),sys.argv[1:3])
if any(flag in sys.argv for flag in ['--fractional','--hotplug']) and '--multi-monitor' not in sys.argv:sys.argv.insert(3,'--multi-monitor')
if '--inside' not in sys.argv:
    assert not ('--clipboard' in sys.argv and '--multi-monitor' in sys.argv),'Run clipboard and monitor fixtures separately'
    assert not ('--ime' in sys.argv and '--multi-monitor' in sys.argv),'Run IME and monitor fixtures separately'
    with tempfile.TemporaryDirectory(prefix='flameshot-kde-') as directory:
        root=Path(directory)
        for name in ['runtime','config','data','cache','tmp','pictures']:(root/name).mkdir(mode=0o700)
        if '--ime' in sys.argv:
            profile=root/'config/fcitx5';profile.mkdir()
            (profile/'profile').write_text('[Groups/0]\nName=Default\nDefault Layout=us\nDefaultIM=pinyin\n[Groups/0/Items/0]\nName=keyboard-us\nLayout=\n[Groups/0/Items/1]\nName=pinyin\nLayout=\n[GroupOrder]\n0=Default\n')
            (root/'config/kwinrc').write_text('[Wayland]\nVirtualKeyboardEnabled=true\n')
        config=root/'config/flameshot';config.mkdir()
        (config/'flameshot.ini').write_text('[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\n')
        applications=root/'data/applications';applications.mkdir()
        (applications/'flameshot-kde-input.desktop').write_text(f'[Desktop Entry]\nType=Application\nName=Isolated test input\nExec={helper}\nX-KDE-Wayland-Interfaces=org_kde_kwin_fake_input,org_kde_plasma_window_management\n')
        portals=root/'config/xdg-desktop-portal';portals.mkdir()
        (portals/'portals.conf').write_text('[preferred]\ndefault=kde\n')
        (root/'config/user-dirs.dirs').write_text(f'XDG_PICTURES_DIR="{root}/pictures"\n')
        bus=root/'bus.conf';bus.write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
        env=os.environ.copy()
        for key in ['DISPLAY','WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS','DBUS_SYSTEM_BUS_ADDRESS','SESSION_MANAGER']:env.pop(key,None)
        env.update(XDG_RUNTIME_DIR=str(root/'runtime'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),TMPDIR=str(root/'tmp'),XDG_CURRENT_DESKTOP='KDE',XDG_SESSION_TYPE='wayland',QT_QPA_PLATFORM='wayland',LIBGL_ALWAYS_SOFTWARE='1',KWIN_COMPOSE='O2')
        session=subprocess.Popen(['dbus-run-session','--config-file',str(bus),'--',sys.executable,__file__,str(binary),str(helper),*[flag for flag in ['--clipboard','--multi-monitor','--fractional','--hotplug','--ime'] if flag in sys.argv],'--inside',str(root)],env=env,start_new_session=True)
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
    compositor=start('kwin',['kwin_wayland','--virtual','--width','1024','--height','768',*(['--output-count','2'] if '--multi-monitor' in sys.argv else []),*(['--inputmethod','/usr/bin/fcitx5 -D --disable=cloudpinyin'] if '--ime' in sys.argv else []),'--no-lockscreen','--no-global-shortcuts','--socket','flameshot-kde'],{'QT_QPA_PLATFORM':'offscreen','QT_LOGGING_RULES':'kwin_core.debug=true;KWIN_UTILS.debug=true'})
    wait_for(lambda:(root/'runtime/flameshot-kde').exists() or compositor.poll() is not None)
    assert compositor.poll() is None,'KWin exited'
    env['WAYLAND_DISPLAY']='flameshot-kde'
    pointer=start('input',[str(helper)],{'FLAMESHOT_TEST_KDE_INPUT':'1'},stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
    assert pointer.stdout.readline().strip()=='ready'
    def key(code):
        for state in [1,0]:
            pointer.stdin.write(f'key {code} {state}\n');pointer.stdin.flush();assert pointer.stdout.readline().strip()=='ok'
    if '--multi-monitor' in sys.argv:
        scale=1.5 if '--fractional' in sys.argv else 2
        logical_width=683 if '--fractional' in sys.argv else 512
        configured=subprocess.run(['kscreen-doctor','output.Virtual-0.scale.1','output.Virtual-0.position.0,0',f'output.Virtual-1.scale.{scale}',f'output.Virtual-1.position.-{logical_width},0'],env=env,capture_output=True,text=True,timeout=10)
        assert configured.returncode==0,configured.stderr+configured.stdout
        layout=subprocess.run(['kscreen-doctor','--json'],env=env,capture_output=True,text=True,check=True,timeout=10)
        (root/'outputs.log').write_text(layout.stdout)
        outputs={output['name']:output for output in json.loads(layout.stdout)['outputs']}
        assert outputs['Virtual-0']['scale']==1 and outputs['Virtual-1']['scale']==scale,outputs
        assert outputs['Virtual-0']['pos']=={'x':logical_width,'y':0} and outputs['Virtual-1']['pos']=={'x':0,'y':0},outputs
    background_code="""from PyQt5.QtWidgets import QApplication,QWidget
app=QApplication([])
windows=[]
for screen in app.screens():
    w=QWidget();w.winId();w.windowHandle().setScreen(screen)
    color='32,64,96' if screen.name()=='Virtual-0' else '160,80,32'
    w.setStyleSheet('background:rgb('+color+')');w.showFullScreen();windows.append(w)
app.exec_()
"""
    background=start('background',['/usr/bin/python3','-c',background_code])
    start('permission-store',['/usr/libexec/xdg-permission-store'])
    start('portal-kde',['/usr/lib/x86_64-linux-gnu/libexec/xdg-desktop-portal-kde'])
    wait_for(lambda:owned('org.freedesktop.impl.portal.desktop.kde'))
    start('portal',['/usr/libexec/xdg-desktop-portal'])
    wait_for(lambda:owned('org.freedesktop.portal.Desktop'))
    time.sleep(1)
    cases=[('full',['full','--raw'],(1024,768),True),('screen',['screen','--number','0','--raw'],(1024,768),True),('editor',['gui','--region','80x60+500+450','--raw'],(80,60),True),('cancel',['gui','--region','80x60+500+450','--raw'],None,False)]
    if '--multi-monitor' in sys.argv:
        cases=[('full',['full','--raw'],(3072,1536),True),('screen',['screen','--number','0','--raw'],(2048,1536),True),('screen-second',['screen','--number','1','--raw'],(1024,768),True),('editor-second',['gui','--region','screen1','--raw'],(1024,768),True),('chooser-second',['gui','--raw'],(1024,768),True),('cancel',['gui','--region','screen1','--raw'],None,False)]
    if '--fractional' in sys.argv:
        cases=[(command,args,((2561,1152) if command=='full' else (1536,1152) if command=='screen' else (1025,768) if size else None),accept) for command,args,size,accept in cases]
    for command,args,size,accept in cases:
        capture=start('capture-'+command,[str(binary),*args],stdout=subprocess.PIPE)
        deadline=time.monotonic()+20
        placement_checked=False
        chosen=False
        while True:
            try:
                data,_=capture.communicate(timeout=.4)
                break
            except subprocess.TimeoutExpired:
                assert time.monotonic()<deadline,'KDE screenshot request timed out'
                if command in ['editor-second','chooser-second']:
                    pointer.stdin.write('geometry org.flameshot.Flameshot\n');pointer.stdin.flush()
                    geometry=pointer.stdout.readline().strip()
                    if geometry!='none':
                        if command=='chooser-second' and not chosen:
                            key(3) # Number 2 chooses the second output.
                            chosen=True
                            continue
                        if geometry!=('0 0 683 512' if '--fractional' in sys.argv else '0 0 512 384'):
                            continue
                        if command=='chooser-second':
                            pointer.stdin.write('key 29 1\n');pointer.stdin.flush();assert pointer.stdout.readline().strip()=='ok'
                            key(30) # Ctrl+A selects the full chosen output.
                            pointer.stdin.write('key 29 0\n');pointer.stdin.flush();assert pointer.stdout.readline().strip()=='ok'
                        placement_checked=True
                key(28 if accept else 1) # Private-session consent/accept or Escape.
        if not accept:
            assert capture.returncode==2 and not data,(capture.returncode,len(data))
            print('PASS: KWin cancellation returns 2 with empty stdout',flush=True)
            continue
        assert capture.returncode==0,(command,capture.returncode)
        if command in ['editor-second','chooser-second']:assert placement_checked,'Editor exited before native geometry could be checked'
        if os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR'):
            artifacts=Path(os.environ['FLAMESHOT_TEST_ARTIFACT_DIR']);artifacts.mkdir(parents=True,exist_ok=True)
            (artifacts/('kde-'+command+'.png')).write_bytes(data)
        image=Image.open(io.BytesIO(data)).convert('RGB')
        assert image.size==size,image.size
        color=(160,80,32) if '--multi-monitor' in sys.argv and command in ['full','screen-second','editor-second','chooser-second'] else (32,64,96)
        assert image.getpixel((20,20))==color,image.getpixel((20,20))
        if '--fractional' in sys.argv and command!='full':
            assert all(image.getpixel((x,y))==color for x in [0,image.width-1] for y in [0,20,image.height-1]),'Monitor crop contains adjacent output pixels'
        print('PASS: KWin/KDE real portal',command,size,'and background pixels',flush=True)
    if '--ime' in sys.argv:
        wait_for(lambda:owned('org.fcitx.Fcitx5'))
        owner=subprocess.run(['gdbus','call','--session','--dest','org.freedesktop.DBus','--object-path','/org/freedesktop/DBus','--method','org.freedesktop.DBus.GetConnectionUnixProcessID','org.fcitx.Fcitx5'],env=env,capture_output=True,text=True,check=True,timeout=5)
        assert owner.stdout.startswith('(uint32 '),owner.stdout
        input_pid=int(owner.stdout.split()[1].rstrip(',)'))
        def remote(*args):
            result=subprocess.run(['fcitx5-remote',*args],env=env,capture_output=True,timeout=5)
            assert result.returncode==0,result.stderr
        def input_command(command):
            pointer.stdin.write(command+'\n');pointer.stdin.flush();assert pointer.stdout.readline().strip()=='ok'
        def control(*codes):
            input_command('key 29 1')
            for code in codes:key(code)
            input_command('key 29 0')
        def copied_text():
            result=subprocess.run(['wl-paste','--type','text/plain;charset=utf-8','--no-newline'],env=env,capture_output=True,timeout=5)
            assert result.returncode==0,result.stderr
            return result.stdout.decode('utf-8')
        editor=start('ime-editor',[str(binary),'gui','--region','400x200+500+450','--raw'],stdout=subprocess.PIPE)
        deadline=time.monotonic()+20
        while True:
            pointer.stdin.write('geometry org.flameshot.Flameshot\n');pointer.stdin.flush()
            if pointer.stdout.readline().strip()=='0 0 1024 768':break
            assert editor.poll() is None and time.monotonic()<deadline
            key(28);time.sleep(.2)
        time.sleep(.5)
        key(20) # Text tool.
        input_command('move 550 500');input_command('button 272 1');input_command('button 272 0')
        time.sleep(.5)
        remote('-s','pinyin');remote('-o');time.sleep(.5)
        for code in [49,23,35,30,24]:key(code);time.sleep(.1) # nihao
        time.sleep(3);key(57);time.sleep(.5)
        remote('-c');control(30,46);time.sleep(.3)
        assert copied_text()=='你好','Pinyin commit did not reach the native editor'
        key(107) # End.
        remote('-o');time.sleep(.3)
        for code in [44,35,24,49,34]:key(code);time.sleep(.1) # zhong
        time.sleep(.5);key(1);time.sleep(.3)
        assert editor.poll() is None,'Cancelling preedit closed the editor'
        remote('-c');control(30,46);time.sleep(.3)
        assert copied_text()=='你好','Cancelling preedit changed the committed text'
        control(28);time.sleep(.3);key(28)
        data,_=editor.communicate(timeout=10)
        assert editor.returncode==0,editor.returncode
        image=Image.open(io.BytesIO(data)).convert('RGB')
        assert image.size==(400,200),image.size
        assert any(r>180 and g<80 and b<80 for r,g,b in image.getdata()),'No text annotation pixels'
        if os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR'):
            Path(os.environ['FLAMESHOT_TEST_ARTIFACT_DIR'],'kde-ime.png').write_bytes(data)
        remote('-e')
        wait_for(lambda:not Path(f'/proc/{input_pid}').exists())
        assert not owned('org.fcitx.Fcitx5')
        assert 'Input Method crashed' not in (root/'kwin.log').read_text()
        print('PASS: KWin native Wayland Pinyin commit, composition cancellation, text copy and annotation export',flush=True)
    if '--hotplug' in sys.argv:
        def geometry():
            pointer.stdin.write('geometry org.flameshot.Flameshot\n');pointer.stdin.flush()
            return pointer.stdout.readline().strip()
        def fullscreen():
            pointer.stdin.write('fullscreen org.flameshot.Flameshot\n');pointer.stdin.flush()
            return pointer.stdout.readline().strip()=='true'
        def configure(*options):
            result=subprocess.run(['kscreen-doctor',*options],env=env,capture_output=True,text=True,timeout=10)
            assert result.returncode==0,result.stderr+result.stdout
        def layout_outputs(name):
            result=subprocess.run(['kscreen-doctor','--json'],env=env,capture_output=True,text=True,check=True,timeout=10)
            (root/(name+'.log')).write_text(result.stdout)
            return [output for output in json.loads(result.stdout)['outputs'] if output['enabled']]
        def finish_capture(process):
            deadline=time.monotonic()+20
            while True:
                try:return process.communicate(timeout=1)[0]
                except subprocess.TimeoutExpired:
                    assert time.monotonic()<deadline,'Hotplug capture did not finish'
                    key(28)
        editor=start('hotplug-editor',[str(binary),'gui','--region','screen1','--raw'],stdout=subprocess.PIPE)
        expected_geometry='0 0 683 512' if '--fractional' in sys.argv else '0 0 512 384'
        deadline=time.monotonic()+20
        while geometry()!=expected_geometry:
            assert editor.poll() is None and time.monotonic()<deadline,'Second-screen editor did not map'
            # Consent can precede the native window. Do not accept an editor
            # whose initial window geometry is still being configured.
            if geometry()=='none':key(28)
            time.sleep(.1)
        configure('output.Virtual-1.disable')
        active=layout_outputs('outputs-disabled')
        assert len(active)==1 and active[0]['name']=='Virtual-0',active
        primary=active[0]
        expected=f"{primary['pos']['x']} {primary['pos']['y']} 1024 768"
        wait_for(lambda:geometry()==expected and fullscreen())
        background.terminate();background.wait(timeout=5)
        key(28)
        data=finish_capture(editor)
        assert editor.returncode==0,editor.returncode
        image=Image.open(io.BytesIO(data)).convert('RGB')
        retained_size=(1025,768) if '--fractional' in sys.argv else (1024,768)
        assert image.size==retained_size,image.size
        assert image.getpixel((20,20))==(160,80,32)
        print('PASS: disabled output moves editor to remaining screen while retaining captured image',flush=True)
        fresh=start('hotplug-single',[str(binary),'full','--raw'],stdout=subprocess.PIPE)
        data=finish_capture(fresh)
        assert fresh.returncode==0,(fresh.returncode,(root/'hotplug-single.log').read_text())
        assert Image.open(io.BytesIO(data)).size==(1024,768)
        invalid=start('hotplug-invalid',[str(binary),'screen','--number','1','--raw'],stdout=subprocess.PIPE)
        data=finish_capture(invalid)
        assert invalid.returncode==1 and not data,(invalid.returncode,len(data))
        assert 'out of range' in (root/'hotplug-invalid.log').read_text()
        configure(f'output.Virtual-0.position.{logical_width},0','output.Virtual-1.enable',f'output.Virtual-1.scale.{scale}','output.Virtual-1.position.0,0')
        assert len(layout_outputs('outputs-restored'))==2
        background=start('background-restored',['/usr/bin/python3','-c',background_code])
        time.sleep(.5)
        restored=start('hotplug-restored',[str(binary),'screen','--number','1','--raw'],stdout=subprocess.PIPE)
        data=finish_capture(restored)
        assert restored.returncode==0,restored.returncode
        image=Image.open(io.BytesIO(data)).convert('RGB')
        assert image.size==retained_size,image.size
        assert image.getpixel((20,20))==(160,80,32),image.getpixel((20,20))
        print('PASS: fresh capture sees disabled output disappear and re-enabled output return',flush=True)
        chooser=start('hotplug-chooser',[str(binary),'gui','--raw'],stdout=subprocess.PIPE)
        deadline=time.monotonic()+20
        while geometry()=='none':
            assert chooser.poll() is None and time.monotonic()<deadline
            key(28);time.sleep(.1)
        time.sleep(.3)
        configure('output.Virtual-1.disable')
        active=layout_outputs('outputs-chooser-disabled')
        assert len(active)==1 and active[0]['name']=='Virtual-0',active
        expected=f"{active[0]['pos']['x']} {active[0]['pos']['y']} 1024 768"
        wait_for(lambda:geometry()==expected)
        key(3) # Select the second screen from the already-captured snapshot.
        time.sleep(.5)
        wait_for(lambda:geometry()==expected and fullscreen(),5)
        pointer.stdin.write('key 29 1\n');pointer.stdin.flush();assert pointer.stdout.readline().strip()=='ok'
        key(30)
        pointer.stdin.write('key 29 0\n');pointer.stdin.flush();assert pointer.stdout.readline().strip()=='ok'
        key(28)
        data=finish_capture(chooser)
        assert chooser.returncode==0,chooser.returncode
        image=Image.open(io.BytesIO(data)).convert('RGB')
        assert image.size==retained_size and image.getpixel((20,20))==(160,80,32)
        print('PASS: selecting a disconnected snapshot screen stays fullscreen on the remaining output',flush=True)

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
