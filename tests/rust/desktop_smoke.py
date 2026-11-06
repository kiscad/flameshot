#!/usr/bin/env python3
"""Real X11 window smoke test; uses only a dedicated Xvfb display and private bus.
Usage: python3 tests/rust/desktop_smoke.py TOOL_ROOT FLAMESHOT_BINARY
TOOL_ROOT contains usr/bin/Xvfb and usr/bin/xdotool (or use /).
"""
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import time

root, binary = map(lambda p: Path(p).resolve(), sys.argv[1:])
artifact_dir=Path(os.environ.get("FLAMESHOT_TEST_ARTIFACT_DIR",str(binary.parent.parent)))
artifact_dir.mkdir(parents=True,exist_ok=True)
env = os.environ.copy()
env.pop('WAYLAND_DISPLAY', None)
env['XDG_SESSION_TYPE'] = 'x11'
env['LD_LIBRARY_PATH'] = str(root / 'usr/lib/x86_64-linux-gnu') + ':' + env.get('LD_LIBRARY_PATH', '')
xvfb_log = tempfile.TemporaryFile()
xvfb = subprocess.Popen([str(root / 'usr/bin/Xvfb'), '-displayfd', '1', '-screen', '0', '1024x768x24', '-nolisten', 'tcp', '-noreset'], stdout=subprocess.PIPE, stderr=xvfb_log, env=env)
process = None
try:
    display = xvfb.stdout.readline().decode().strip()
    if not display:
        xvfb_log.seek(0)
        raise AssertionError(xvfb_log.read().decode())
    env['DISPLAY'] = ':' + display
    with tempfile.TemporaryDirectory(prefix='flameshot-desktop-') as directory:
        bus_config = Path(directory) / 'bus.conf'
        bus_config.write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
        bus_command = ['dbus-run-session', '--config-file', str(bus_config), '--']
        runtime=Path(directory)/'runtime';runtime.mkdir(mode=0o700)
        env['XDG_RUNTIME_DIR']=str(runtime)
        env['XDG_CONFIG_HOME'] = directory
        env['XDG_CACHE_HOME'] = directory
        test_config=Path(directory)/'flameshot/flameshot.ini'
        test_config.parent.mkdir()
        test_config.write_text('[General]\ncheckForUpdates=false\n')
        def tool(*args):
            return subprocess.run([str(root / 'usr/bin/xdotool'), *args], env=env, capture_output=True, timeout=10)
        def png(data):
            assert data[:8] == b'\x89PNG\r\n\x1a\n', data[:200]
            return struct.unpack('>II', data[16:24])
        captured = subprocess.run(bus_command + [str(binary), 'full', '--raw'], env=env, capture_output=True, timeout=25)
        assert captured.returncode == 0, captured.stderr.decode()
        assert png(captured.stdout) == (1024, 768)
        for flags in (['screen', '--number', '0', '--raw'], ['screen', '--raw']):
            screen_capture = subprocess.run(bus_command + [str(binary), *flags], env=env, capture_output=True, timeout=25)
            assert screen_capture.returncode == 0, screen_capture.stderr.decode()
            assert png(screen_capture.stdout) == (1024, 768)
        cropped = subprocess.run(bus_command+[str(binary),'screen','-n','0','--region','40x30+10+20','--raw'],env=env,capture_output=True,timeout=25)
        assert cropped.returncode==0,cropped.stderr.decode()
        assert png(cropped.stdout)==(40,30)
        invalid = subprocess.run(bus_command + [str(binary), 'screen', '--number', '999', '--raw'], env=env, capture_output=True, timeout=10)
        assert invalid.returncode == 1 and invalid.stdout == b''
        assert b'out of range' in invalid.stderr
        clipboard_check = r"""
import subprocess,sys,struct,time,os
from pathlib import Path
binary,xclip,xdotool=sys.argv[1:]
result=subprocess.run([binary,'full','--clipboard'],capture_output=True,timeout=20)
assert result.returncode==0,result.stderr.decode()
assert not result.stdout
copied=subprocess.run([xclip,'-selection','clipboard','-t','image/png','-o'],capture_output=True,timeout=5)
assert copied.returncode==0,copied.stderr.decode()
assert copied.stdout[:8]==b'\x89PNG\r\n\x1a\n'
assert struct.unpack('>II',copied.stdout[16:24])==(1024,768)
print('PASS: clipboard image survives capture process exit')
def tool(*args):
    return subprocess.run([xdotool,*args],capture_output=True,timeout=5)
def wait_window(title):
    deadline=time.monotonic()+10
    while time.monotonic()<deadline:
        found=tool('search','--onlyvisible','--name',title)
        if found.returncode==0:return found.stdout.decode().splitlines()[-1]
        time.sleep(.1)
    raise AssertionError('Missing window '+title)
config=Path(os.environ['XDG_CONFIG_HOME'])/'flameshot/flameshot.ini'
original=config.read_text()
saved_folder=config.parent/'saved images';saved_folder.mkdir()
try:
    config.write_text(f'[General]\ncheckForUpdates=false\nsaveAfterCopy=true\ncopyPathAfterSave=true\nsavePath={saved_folder}\nfilenamePattern=copy-save\n')
    result=subprocess.run([binary,'full','--clipboard'],capture_output=True,timeout=20)
    assert result.returncode==0,result.stderr.decode()
    assert (saved_folder/'copy-save.png').exists()
    copied=subprocess.run([xclip,'-selection','clipboard','-t','image/png','-o'],capture_output=True,timeout=5)
    assert copied.returncode==0 and copied.stdout[:8]==b'\x89PNG\r\n\x1a\n','Path replaced copied image'
    destination=saved_folder/'explicit.png'
    result=subprocess.run([binary,'full','--path',str(destination)],capture_output=True,timeout=20)
    assert result.returncode==0,result.stderr.decode()
    copied=subprocess.run([xclip,'-selection','clipboard','-t','UTF8_STRING','-o'],capture_output=True,timeout=5)
    assert copied.returncode==0 and copied.stdout.decode()==str(destination),copied.stderr.decode()
    config.write_text('[General]\ncheckForUpdates=false\nsaveAfterCopy=true\nfilenamePattern=fallback\n')
    result=subprocess.run([binary,'full','--clipboard'],env=dict(os.environ,HOME=str(saved_folder)),capture_output=True,timeout=20)
    assert result.returncode==0,result.stderr.decode()
    assert (saved_folder/'fallback.png').exists(),'Missing savePath did not use default directory'
    config.write_text(f'[General]\ncheckForUpdates=false\nsaveAfterCopy=true\nsavePath={saved_folder}/missing/directory\n')
    result=subprocess.run([binary,'full','--clipboard'],capture_output=True,timeout=20)
    assert result.returncode==0 and b'Cannot save image after copy' in result.stderr,result.stderr.decode()
    copied=subprocess.run([xclip,'-selection','clipboard','-t','image/png','-o'],capture_output=True,timeout=5)
    assert copied.returncode==0 and copied.stdout[:8]==b'\x89PNG\r\n\x1a\n','Failed autosave lost clipboard image'
finally:
    config.write_text(original)
print('PASS: copy saves image, save copies path, image wins when both enabled, failed autosave still copies')
def service_owned():
    result=subprocess.run(['gdbus','call','--session','--dest','org.freedesktop.DBus','--object-path','/org/freedesktop/DBus','--method','org.freedesktop.DBus.NameHasOwner','org.flameshot.Flameshot'],capture_output=True,timeout=5)
    return result.returncode==0 and b'true' in result.stdout

def wait_idle_exit():
    deadline=time.monotonic()+8
    while service_owned():
        assert time.monotonic()<deadline,'Idle daemon did not exit after releasing resources'
        time.sleep(.1)

try:
    config.write_text('[General]\ncheckForUpdates=false\nuseJpgForClipboard=true\n')
    result=subprocess.run([binary,'full','--clipboard'],capture_output=True,timeout=20)
    assert result.returncode==0,result.stderr.decode()
    jpeg=subprocess.run([xclip,'-selection','clipboard','-t','image/jpeg','-o'],capture_output=True,timeout=5)
    assert jpeg.returncode==0 and jpeg.stdout[:2]==bytes([255,216]),jpeg.stderr.decode()
    import io,random
    from PIL import Image
    assert Image.open(io.BytesIO(jpeg.stdout)).size==(1024,768)
    config.write_text('[General]\ncheckForUpdates=false\nautoCloseIdleDaemon=true\n')
    pattern=random.Random(7).randbytes(1024*768//8)
    bitmap=config.parent/'noise.xbm'
    bitmap.write_text('#define noise_width 1024\n#define noise_height 768\nstatic unsigned char noise_bits[] = {\n'+','.join(format(value,'#04x') for value in pattern)+'};\n')
    subprocess.run(['xsetroot','-fg','white','-bg','black','-bitmap',str(bitmap)],check=True,timeout=5)
    expected=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=20)
    assert expected.returncode==0 and len(expected.stdout)>32768,expected.stderr.decode()
    result=subprocess.run([binary,'full','--clipboard'],capture_output=True,timeout=20)
    assert result.returncode==0,result.stderr.decode()
    time.sleep(1.2)
    assert service_owned(),'Idle daemon exited while owning clipboard'
    copied=subprocess.run([xclip,'-selection','clipboard','-t','image/png','-o'],capture_output=True,timeout=10)
    assert copied.returncode==0 and copied.stdout==expected.stdout,'Large INCR clipboard transfer differs from capture'
    pending_file=config.parent/'pending-paste.png'
    probe=subprocess.Popen([str(Path(os.environ.get('FLAMESHOT_TEST_HELPER_DIR',str(Path(binary).parent/'examples')))/'x11_clipboard_probe'),str(pending_file)],env=dict(os.environ,FLAMESHOT_TEST_X11_INPUT='1'),stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    try:
        import select
        assert select.select([probe.stdout],[],[],5)[0],'Clipboard probe did not start'
        assert probe.stdout.readline().strip()=='ready','Clipboard probe did not receive INCR'
        config.write_text('[General]\ncheckForUpdates=false\nautoCloseIdleDaemon=true\nuseJpgForClipboard=true\n')
        result=subprocess.run([binary,'full','--clipboard'],capture_output=True,timeout=20)
        assert result.returncode==0,result.stderr.decode()
        subprocess.run([xclip,'-selection','clipboard','-loops','1'],input=b'replacement',stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=True,timeout=5)
        time.sleep(1.2)
        assert service_owned(),'Idle daemon abandoned an older in-flight paste'
        try:out,err=probe.communicate('read\n',timeout=10)
        except subprocess.TimeoutExpired as error:raise AssertionError(f'INCR probe timed out: {error.stderr!r}') from error
        assert probe.returncode==0 and 'done' in out,err
        assert pending_file.read_bytes()==expected.stdout,'In-flight paste lost data after ownership changed'
    finally:
        if probe.poll() is None:probe.kill();probe.wait()
    wait_idle_exit()
    subprocess.run(['xsetroot','-solid','black'],check=True,timeout=5)
    result=subprocess.run([binary,'screen','--number','0','--pin'],capture_output=True,timeout=20)
    assert result.returncode==0,result.stderr.decode()
    window=wait_window('^flameshot-pin$')
    time.sleep(1.2)
    assert service_owned(),'Idle daemon exited while hosting a pin'
    tool('windowfocus','--sync',window);time.sleep(.2);tool('key','Escape')
    wait_idle_exit()
finally:
    config.write_text(original)
    subprocess.run(['xsetroot','-solid','black'],timeout=5)
print('PASS: JPEG MIME, large INCR clipboard, ownership-aware idle exit and pin lifetime')
editor=subprocess.Popen([binary,'gui'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
try:
    window=wait_window('^Flameshot$')
    duplicate=subprocess.run([binary,'gui','--raw'],capture_output=True,timeout=5)
    assert duplicate.returncode==1 and b'already running' in duplicate.stderr,duplicate.stderr.decode()
    assert not duplicate.stdout
    config=Path(os.environ['XDG_CONFIG_HOME'])/'flameshot/flameshot.ini'
    original=config.read_text()
    config.write_text(original+'\n[General]\nallowMultipleGuiInstances=true\n')
    second=subprocess.Popen([binary,'gui','--raw'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        deadline=time.monotonic()+10
        while True:
            found=tool('search','--all','--onlyvisible','--pid',str(second.pid),'--name','^Flameshot$')
            if found.returncode==0:break
            assert time.monotonic()<deadline,'Multiple editors setting was not honored'
            time.sleep(.1)
        tool('windowfocus','--sync',found.stdout.decode().splitlines()[-1])
        time.sleep(.3);tool('key','Escape')
        out,err=second.communicate(timeout=10);assert second.returncode==2,err.decode()
    finally:
        config.write_text(original)
        if second.poll() is None:second.kill();second.wait()
    print('PASS: capture lock rejects duplicates and honors multiple-editor setting')
    tool('windowsize' ,'--sync',window,'1024','768')
    tool('windowfocus','--sync',window)
    time.sleep(.4)
    tool('mousemove','--window',window,'100','180')
    tool('mousedown','1')
    tool('mousemove','--window',window,'300','380')
    time.sleep(.2)
    tool('mouseup','1')
    time.sleep(.2)
    tool('mousemove','--window',window,'200','300')
    tool('click','--repeat','2','--delay','80','1')
    time.sleep(.3)
    assert editor.poll() is None,'Disabled double click unexpectedly copied'
    config.write_text(f'[General]\ncheckForUpdates=false\nsaveAfterCopy=true\ncopyPathAfterSave=true\nsavePath={saved_folder}\nfilenamePattern=gui-copy\n')
    tool('key','ctrl+c')
    out,err=editor.communicate(timeout=10)
    assert editor.returncode==0,err.decode()
    assert not out
    copied=subprocess.run([xclip,'-selection','clipboard','-t','image/png','-o'],capture_output=True,timeout=5)
    assert copied.returncode==0,copied.stderr.decode()
    assert struct.unpack('>II',copied.stdout[16:24])==(200,200)
    assert struct.unpack('>II',(saved_folder/'gui-copy.png').read_bytes()[16:24])==(200,200)
    config.write_text(original)
finally:
    if editor.poll() is None:editor.kill();editor.wait()
print('PASS: editor Ctrl+C preserves cropped clipboard after closing')
config.write_text('[General]\ncheckForUpdates=false\ncopyPathAfterSave=true\n')
save_editor=subprocess.Popen([binary,'gui','--region','40x30+20+30','--path',str(saved_folder/'explicit.png')],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
try:
    window=wait_window('^Flameshot$')
    tool('windowfocus','--sync',window)
    time.sleep(.3)
    tool('key','Return')
    out,err=save_editor.communicate(timeout=10)
    assert save_editor.returncode==0,err.decode()
    actual=saved_folder/'explicit_1.png'
    assert struct.unpack('>II',actual.read_bytes()[16:24])==(40,30)
    copied=subprocess.run([xclip,'-selection','clipboard','-t','UTF8_STRING','-o'],capture_output=True,timeout=5)
    assert copied.returncode==0 and copied.stdout.decode()==str(actual),copied.stderr.decode()
finally:
    config.write_text(original)
    if save_editor.poll() is None:save_editor.kill();save_editor.wait()
print('PASS: GUI save copies actual collision-resolved path without duplicate save')
config.write_text('[General]\ncheckForUpdates=false\ncopyOnDoubleClick=true\n')
double_editor=subprocess.Popen([binary,'gui','--region','200x200+100+180'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
try:
    window=wait_window('^Flameshot$')
    tool('windowsize','--sync',window,'1024','768')
    tool('windowfocus','--sync',window)
    time.sleep(.3)
    tool('mousemove','--window',window,'600','600')
    tool('click','--repeat','2','--delay','80','1')
    time.sleep(.3)
    assert double_editor.poll() is None,'Double click outside selection copied the capture'
    tool('mousemove','--window',window,'200','300')
    tool('click','--repeat','2','--delay','80','1')
    out,err=double_editor.communicate(timeout=10)
    assert double_editor.returncode==0,err.decode()
    copied=subprocess.run([xclip,'-selection','clipboard','-t','image/png','-o'],capture_output=True,timeout=5)
    assert copied.returncode==0 and struct.unpack('>II',copied.stdout[16:24])==(200,200),copied.stderr.decode()
finally:
    config.write_text(original)
    if double_editor.poll() is None:double_editor.kill();double_editor.wait()
print('PASS: enabled double-click copies only inside selection and persists clipboard')
pinned=subprocess.run([binary,'screen','--number','0','--pin'],capture_output=True,timeout=20)
assert pinned.returncode==0,pinned.stderr.decode()
window=wait_window('^flameshot-pin$')
tool('windowfocus','--sync',window)
time.sleep(.3)
def geometry():
    result=tool('getwindowgeometry','--shell',window)
    assert result.returncode==0,result.stderr.decode()
    fields=dict(line.split('=',1) for line in result.stdout.decode().splitlines())
    return int(fields['WIDTH']),int(fields['HEIGHT'])
base=geometry()
tool('mousemove','--window',window,'100','100')
tool('click','4')
time.sleep(.4)
zoomed=geometry()
assert zoomed[0]>base[0] and zoomed[1]>base[1],(base,zoomed)
assert abs(zoomed[0]*base[1]-zoomed[1]*base[0])<=sum(base),(base,zoomed)
tool('click','--repeat','2','--delay','80','1')
time.sleep(.4)
assert geometry()==base,(base,geometry())
tool('key','5')
tool('key','0')
tool('key','Escape')
time.sleep(.3)
assert tool('search','--onlyvisible','--name','^flameshot-pin$').returncode!=0
print('PASS: D-Bus pin zoom preserves aspect, resets, and closes')
launcher=subprocess.Popen([binary,'launcher'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
try:
    window=wait_window('^Flameshot Launcher$')
    tool('windowfocus','--sync',window)
    time.sleep(.3)
    tool('key','Return')
    editor_window=wait_window('^Flameshot$')
    tool('windowfocus','--sync',editor_window)
    time.sleep(.3)
    tool('key','Escape')
    window=wait_window('^Flameshot Launcher$')
    tool('windowfocus','--sync',window)
    time.sleep(.3)
    tool('key','Escape')
    out,err=launcher.communicate(timeout=10)
    assert launcher.returncode==0,err.decode()
finally:
    if launcher.poll() is None:launcher.kill();launcher.wait()
print('PASS: launcher hides for capture and returns after cancellation')
crashed=subprocess.Popen([binary,'gui','--raw'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
try:
    wait_window('^Flameshot$')
finally:
    crashed.kill();crashed.wait()
reopened=subprocess.Popen([binary,'gui','--raw'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
try:
    window=wait_window('^Flameshot$')
    tool('windowfocus','--sync',window);time.sleep(.3);tool('key','Escape')
    out,err=reopened.communicate(timeout=10);assert reopened.returncode==2,err.decode()
finally:
    if reopened.poll() is None:reopened.kill();reopened.wait()
print('PASS: capture lock is released after a killed editor')
"""
        copied = subprocess.run(bus_command + [sys.executable, '-c', clipboard_check, str(binary), str(root / 'usr/bin/xclip'), str(root / 'usr/bin/xdotool')], env=env, capture_output=True, timeout=50)
        assert copied.returncode == 0, copied.stderr.decode()
        print(copied.stdout.decode().strip())
        help_text=subprocess.run([str(binary),'--help'],env=env,capture_output=True,timeout=5).stdout
        if b'history' in help_text:
            history=Path(directory)/'flameshot/history'
            history.mkdir(parents=True,exist_ok=True)
            fixture=Path(__file__).resolve().parents[2]/'crates/flameshot-core/tests/fixtures/qt6-clipboard.bin'
            (history/'imgur-testtoken-abc.png').write_bytes(fixture.read_bytes()[4:])
            process=subprocess.Popen(bus_command+[str(binary),'history'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
            deadline=time.monotonic()+15
            while True:
                found=tool('search','--onlyvisible','--name','^Flameshot Upload History$')
                if found.returncode==0:break
                assert time.monotonic()<deadline,'Upload history window did not appear'
                time.sleep(.1)
            tool('windowfocus','--sync',found.stdout.decode().splitlines()[-1])
            time.sleep(.3)
            tool('key','Escape')
            out,err=process.communicate(timeout=10)
            assert process.returncode==0,err.decode()
            process=None
            assert (history/'imgur-testtoken-abc.png').exists()
            print('PASS: optional upload history opens legacy local record without modifying it')
        process = subprocess.Popen(bus_command + [str(binary), 'config'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        deadline=time.monotonic()+15
        while True:
            found=tool('search','--onlyvisible','--name','^Flameshot Configuration$')
            if found.returncode==0:break
            assert time.monotonic()<deadline,'Configuration window did not appear'
            time.sleep(.1)
        window=found.stdout.decode().splitlines()[-1]
        tool('windowfocus','--sync',window)
        time.sleep(.2)
        tool('key','Escape')
        out,err=process.communicate(timeout=10)
        assert process.returncode==0,err.decode()
        assert (Path(directory)/'flameshot/flameshot.ini').read_text()=='[General]\ncheckForUpdates=false\n'
        process=None
        print('PASS: configuration window opens and closes without modifying settings')
        for mode in ('cancel','raw','geometry','both','autosave','adjust','magnifier','save-retry','save-cancel','preset','last','lastconfig','lastoverride','fixed','shortcut','shortcutmeta','shortcutdefault','openwith','toolsize','undolimit','color','opacity','textedit','objectmove','palette','uploadcancel','buttons','geometryoverlay','pixelatesafe','pixelateinsecure','quitresume','quitconfirm','quitdisable','monitors','monitoroverride','monitordelay'):
            if mode=='uploadcancel' and b'history' not in help_text: continue
            cancel = mode in ('cancel','quitconfirm','quitdisable')
            flags = ['--raw'] if mode in ('cancel','raw') else ['--accept-on-select','--print-geometry']
            if mode in ('pixelatesafe','pixelateinsecure'):
                flags=['--raw']
                test_config.write_text('[General]\ncheckForUpdates=false\nshowSelectionGeometry=0\ninsecurePixelate='+str(mode=='pixelateinsecure').lower()+'\n')
                subprocess.run(['xsetroot','-solid','white'],env=env,check=True,timeout=5)
            if mode=='geometryoverlay':
                flags=['--raw','--region','200x200+100+180']
                test_config.write_text('[General]\ncheckForUpdates=false\nshowSelectionGeometry=5\nshowSelectionGeometryHideTime=1500\nshowSidePanelButton=false\n')
                subprocess.run(['xsetroot','-solid','white'],env=env,check=True,timeout=5)
            if mode=='uploadcancel':
                flags=['--raw']
                test_config.write_text('[General]\ncheckForUpdates=false\nuploadWithoutConfirmation=false\n[Shortcuts]\nTYPE_IMAGEUPLOADER=F11\n')
            if mode in ('quitresume','quitconfirm','quitdisable'):
                flags=['--raw','--region','200x200+100+180']
                test_config.write_text('[General]\ncheckForUpdates=false\nshowQuitPrompt=true\nshowAbortNotification=false\n')
            if mode=='monitors':
                test_config.write_text('[General]\ncheckForUpdates=false\ncaptureActiveMonitor=true\nuseX11LegacyScreenshot=true\n')
                for name,bounds,output in [('left','512/135x768/203+0+0','screen'),('right','512/135x768/203+512+0','none')]:
                    subprocess.run(['xrandr','--setmonitor',name,bounds,output],env=env,check=True,capture_output=True,timeout=5)
                bitmap=Path(directory)/'monitors.xbm'
                bitmap.write_text('#define colors_width 1024\n#define colors_height 1\nstatic unsigned char colors_bits[] = {\n'+','.join(['0xff']*64+['0x00']*64)+'};\n')
                subprocess.run(['xsetroot','-fg','blue','-bg','red','-bitmap',str(bitmap)],env=env,check=True,timeout=5)
                tool('mousemove','750','300')
            if mode in ('monitors','monitoroverride','monitordelay'):
                flags=['--raw','--region','screen0' if mode=='monitoroverride' else 'all']
            if mode=='monitordelay':
                flags += ['--delay','800']
                tool('mousemove','200','300')
            if mode=='buttons':
                flags=['--raw']
                ids=[21,23,20]
                data=struct.pack('>II',127,11)+b'QList<int>\0'+struct.pack('>I',len(ids))+b''.join(struct.pack('>i',value) for value in ids)
                variant='@Variant('+''.join(chr(92)+'x'+format(byte,'02x') for byte in data)+')'
                test_config.write_text('[General]\ncheckForUpdates=false\ndrawThickness=3\nbuttons='+variant+'\n')
            if mode=='palette':
                flags=['--raw']
                test_config.write_text('[General]\ncheckForUpdates=false\nuserColors=#00ff00, red, picker\n')
            if mode=='objectmove':
                flags=['--raw','--region','400x200+500+450']
                data=struct.pack('>II',127,11)+b'QList<int>\0'+struct.pack('>I',0)
                variant='@Variant('+''.join(chr(92)+'x'+format(byte,'02x') for byte in data)+')'
                test_config.write_text('[General]\ncheckForUpdates=false\nlanguage=en\nuserColors=#00ff00, #0000ff, #ff0000\nshowSelectionGeometry=0\nbuttons='+variant+'\n')
            if mode=='textedit':
                flags=['--raw','--region','400x200+500+450']
                test_config.write_text('[General]\ncheckForUpdates=false\ncopyOnDoubleClick=true\ndrawFontSize=18\nshowSelectionGeometry=0\n')
            if mode=='opacity':
                flags=['--raw']
                test_config.write_text('[General]\ncheckForUpdates=false\ncontrastOpacity=255\nshowSelectionGeometry=0\nuiColor=#00ffff\ncontrastUiColor=#ff0000\n')
                subprocess.run(['xsetroot','-solid','white'],env=env,check=True,timeout=5)
            if mode=='color':
                flags=['--raw']
                test_config.write_text('[General]\ncheckForUpdates=false\ndrawColor=#00ff0080\nfontFamily=DejaVu Serif\n')
            if mode=='undolimit':
                flags=['--raw']
                test_config.write_text('[General]\ncheckForUpdates=false\nundoLimit=1\n')
            if mode=='toolsize':
                flags=['--raw']
                test_config.write_text('[General]\ncheckForUpdates=false\nundoLimit=1\ndrawThickness=3\ndrawMarkerSize=8\ndrawPixelateSize=2\ndrawCircleCounterSize=1\ndrawRectangleSize=4\ndrawFontSize=12\n[Shortcuts]\nTYPE_SIZEINCREASE=F9\nTYPE_CIRCLECOUNT=F10\n')
            if mode=='openwith':
                flags=['--raw']
                (Path(directory)/'flameshot/flameshot.ini').write_text('[General]\ncheckForUpdates=false\n')
            if mode in ('shortcut','shortcutmeta','shortcutdefault'):
                flags=['--raw']
                binding={'shortcut':'F8','shortcutmeta':'Meta+F8','shortcutdefault':'Ctrl+S'}[mode]
                (Path(directory)/'flameshot/flameshot.ini').write_text(f'[General]\ncheckForUpdates=false\n[Shortcuts]\nTYPE_ACCEPT={binding}\n')
            if mode == 'fixed':
                flags=['--raw']
                (Path(directory)/'flameshot/flameshot.ini').write_text(f'[General]\ncheckForUpdates=false\nsavePathFixed=true\nsavePath={directory}\nfilenamePattern=fixed\n')
            if mode == 'preset': flags=['--region','80x60+20+30','--print-geometry']
            if mode=='lastconfig':
                flags=['--print-geometry']
                test_config.write_text('[General]\ncheckForUpdates=false\nsaveLastRegion=true\n')
            if mode=='lastoverride': flags=['--region','20x10+5+6','--print-geometry']
            if mode == 'last': flags=['--last-region','--print-geometry']
            if mode == 'both': flags += ['--raw']
            if mode == 'adjust': flags = ['--print-geometry']
            if mode in ('save-retry','save-cancel'): flags = ['--raw']
            if mode == 'magnifier':
                flags = ['--raw']
                config = Path(directory)/'flameshot/flameshot.ini'
                config.parent.mkdir(exist_ok=True)
                config.write_text('[General]\nshowMagnifier=true\nuiLanguage=en\ncheckForUpdates=false\n')
            if mode == 'autosave': flags = ['--accept-on-select','--path',str(Path(directory)/'auto.png')]
            process = subprocess.Popen(bus_command + [str(binary), 'gui', *flags], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            if mode=='monitordelay':
                time.sleep(.3)
                tool('mousemove','750','300')
            deadline = time.monotonic() + 25
            window = ''
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise AssertionError(process.communicate()[1].decode())
                found = tool('search', '--onlyvisible', '--name', '^Flameshot$')
                if found.returncode == 0:
                    window = found.stdout.decode().splitlines()[-1]
                    break
                time.sleep(0.1)
            assert window, 'Editor did not display a visible window'
            window_pid=tool('getwindowpid',window).stdout.decode().strip()
            expected_class='org.flameshot.Flameshot' if env.get('FLATPAK_ID') or Path('/.flatpak-info').exists() else 'flameshot'
            identity=tool('search','--all','--pid',window_pid,'--class','^'+expected_class.replace('.','[.]')+'$')
            assert identity.returncode==0,'Editor WM_CLASS does not match desktop entry'

            # Xvfb has no window manager to honor fullscreen requests.
            assert tool('windowsize', '--sync', window, '1024', '768').returncode == 0
            assert tool('windowfocus', '--sync', window).returncode == 0
            time.sleep(0.5)
            if mode == 'magnifier':
                tool('mousemove','--window',window,'400','500')
                time.sleep(.3)
                artifact = subprocess.run(bus_command+[str(binary),'full','--raw'],env=env,capture_output=True,timeout=20)
                assert artifact.returncode==0,artifact.stderr.decode()
                (artifact_dir/'rust-magnifier-x11.png').write_bytes(artifact.stdout)
                tool('key','g')
            if mode in ('quitresume','quitconfirm','quitdisable'):
                if mode=='quitresume':
                    tool('key','p')
                    time.sleep(.15)
                    tool('key','Escape')
                    time.sleep(.15)
                tool('key','ctrl+q' if mode=='quitconfirm' else 'Escape')
                time.sleep(.3)
                assert process.poll() is None,'Quit prompt did not keep capture alive'
                if mode=='quitconfirm':
                    tool('key','Tab')
                    time.sleep(.15)
                    tool('key','Return')
                else:
                    if mode=='quitdisable':
                        tool('key','shift+Tab')
                        time.sleep(.15)
                        tool('key','space')
                        time.sleep(.15)
                        tool('key','Tab')
                    tool('key','Return')
                    time.sleep(.3)
                    assert process.poll() is None,'Default No closed capture'
                    if mode=='quitdisable':
                        import configparser
                        saved=configparser.ConfigParser();saved.read(test_config)
                        assert saved.get('General','showQuitPrompt').strip(chr(34))=='false','Do not show again was not persisted'
                        tool('key','Escape')
                    else: tool('key','Return')
            elif mode=='geometryoverlay':
                import io
                from PIL import Image
                def geometry_pixels():
                    result=subprocess.run(bus_command+[str(binary),'full','--raw'],env=env,capture_output=True,timeout=20)
                    assert result.returncode==0,result.stderr.decode()
                    return Image.open(io.BytesIO(result.stdout)).convert('RGB').crop((180,270,220,290)).tobytes()
                visible=geometry_pixels()
                assert any(value!=255 for value in visible),'Geometry display did not appear'
                time.sleep(1.7)
                assert all(value==255 for value in geometry_pixels()),'Geometry timeout did not hide overlay'
                tool('key','Right')
                time.sleep(.15)
                assert any(value!=255 for value in geometry_pixels()),'Selection movement did not restore geometry'
                tool('key','Return')
            elif mode in ('monitors','monitoroverride','monitordelay'):
                tool('key','Return')
            elif mode=='objectmove':
                import io
                from PIL import Image
                def move_pixels():
                    time.sleep(.2)
                    captured=subprocess.run(bus_command+[str(binary),'full','--raw'],env=env,capture_output=True,timeout=20)
                    assert captured.returncode==0,captured.stderr.decode()
                    (artifact_dir/'rust-object-move-x11.png').write_bytes(captured.stdout)
                    image=Image.open(io.BytesIO(captured.stdout)).convert('RGB')
                    return image.getpixel((550,500)),image.getpixel((650,550))
                background=move_pixels()
                tool('key','r')
                time.sleep(.2)
                tool('mousemove','--window',window,'530','480')
                tool('mousedown','1')
                time.sleep(.15)
                tool('mousemove','--window',window,'570','520')
                time.sleep(.2)
                tool('mouseup','1')
                time.sleep(.2)
                # An outline above the filled rectangle must not intercept its center.
                tool('key','s')
                time.sleep(.2)
                tool('mousemove','--window',window,'520','470')
                tool('mousedown','1')
                time.sleep(.15)
                tool('mousemove','--window',window,'580','530')
                time.sleep(.2)
                tool('mouseup','1')
                original=move_pixels()
                assert original[0][0]>original[0][1]+80,original
                tool('mousemove','--window',window,'180','37')
                tool('click','1') # The reduced toolbar contains Capture region and Move object.
                time.sleep(.2)
                def drag_object():
                    tool('mousemove','--window',window,'550','500')
                    tool('mousedown','1')
                    time.sleep(.15)
                    tool('mousemove','--window',window,'650','550')
                drag_object()
                preview=move_pixels()
                assert preview[0]==background[0] and preview[1]==original[0],(preview,original,background)
                tool('mouseup','1')
                assert move_pixels()==preview,'Move preview changed on release'
                tool('key','ctrl+z')
                assert move_pixels()==original,'Moving was not one undoable edit'
                tool('key','ctrl+shift+z')
                assert move_pixels()==preview,'Redo failed to restore movement'
                tool('key','ctrl+z')
                drag_object()
                assert move_pixels()==preview
                tool('key','Escape')
                tool('mouseup','1')
                assert move_pixels()==original,'Cancelling a move changed the document'
                tool('key','ctrl+shift+z')
                assert move_pixels()==preview,'Cancelled drag erased redo history'
                tool('mousemove','--window',window,'180','37')
                tool('click','1')
                time.sleep(.2)
                tool('mousemove','--window',window,'650','550')
                tool('click','1')
                time.sleep(.2)
                tool('mousemove','--window',window,'40','60')
                tool('click','1') # Green in the configured three-color palette.
                recolored=move_pixels()
                assert recolored[0]==preview[0] and recolored[1]==(0,255,0),recolored
                tool('key','ctrl+z')
                assert move_pixels()==preview,'Recoloring did not undo in one step'
                tool('key','ctrl+shift+z')
                assert move_pixels()==recolored,'Recoloring redo failed'
                tool('mousemove','--window',window,'650','550')
                tool('click','1')
                time.sleep(.2)
                tool('key','p')
                time.sleep(.2)
                tool('mousemove','--window',window,'77','60')
                tool('click','1') # Blue becomes the drawing default, not an object edit.
                assert move_pixels()==recolored,'Drawing color changed a previously selected object'
                tool('key','Return')
            elif mode=='textedit':
                import io
                from PIL import Image
                def text_pixels():
                    time.sleep(.3)
                    result=subprocess.run(bus_command+[str(binary),'full','--raw'],env=env,capture_output=True,timeout=20)
                    assert result.returncode==0,result.stderr.decode()
                    return Image.open(io.BytesIO(result.stdout)).convert('RGB').crop((520,480,880,620)).tobytes()
                tool('key','t')
                time.sleep(.3)
                tool('mousemove','--window',window,'550','500')
                tool('click','1')
                time.sleep(.4)
                tool('type','--clearmodifiers','Original')
                tool('key','ctrl+Return')
                time.sleep(.4)
                assert process.poll() is None,'Creating text closed the editor'
                tool('key','p')
                original_pixels=text_pixels()
                assert any(original_pixels[i]>original_pixels[i+1]+50 and original_pixels[i]>original_pixels[i+2]+50 for i in range(0,len(original_pixels),3)),'No text was rendered'
                def edit_text(value):
                    tool('mousemove','--window',window,'560','505')
                    tool('click','--repeat','2','--delay','80','1')
                    time.sleep(.4)
                    assert process.poll() is None,'Double click copied text instead of editing it'
                    tool('key','ctrl+a')
                    tool('type','--clearmodifiers',value)
                edit_text('Cancelled')
                assert text_pixels()!=original_pixels,'Draft text did not preview before Apply'
                tool('key','Escape')
                time.sleep(.3)
                assert process.poll() is None,'Cancelling text closed capture'
                assert text_pixels()==original_pixels,'Cancelled text mutated the annotation'
                edit_text('Edited')
                draft_pixels=text_pixels()
                tool('key','ctrl+Return')
                edited_pixels=text_pixels()
                assert edited_pixels!=original_pixels,'Applying text did not change pixels'
                assert edited_pixels==draft_pixels,'Committed pixels differ from the text preview'
                tool('key','ctrl+z')
                assert text_pixels()==original_pixels,'Text edit was not one undoable replacement'
                tool('key','ctrl+shift+z')
                assert text_pixels()==edited_pixels,'Redo failed to restore edited text'
                tool('key','Return')
            elif cancel:
                tool('key', 'Escape')
            elif mode in ('preset','last','lastconfig','lastoverride'):
                tool('key','Return')
            else:
                tool('mousemove', '--window', window, '100', '180')
                tool('mousedown', '1')
                tool('mousemove', '--window', window, '300', '380')
                time.sleep(0.2)
                tool('mouseup', '1')
                time.sleep(0.2)
                if mode == 'adjust':
                    tool('key','Right')
                    tool('key','shift+Right')
                    tool('key','ctrl+m')
                    time.sleep(.2)
                    tool('mousemove','--window',window,'150','230')
                    tool('mousedown','1')
                    tool('mousemove','--window',window,'170','260')
                    time.sleep(.2)
                    tool('mouseup','1')
                    time.sleep(.2)
                    tool('key','s')
                    time.sleep(.2)
                    tool('mousemove','--window',window,'322','410')
                    tool('mousedown','1')
                    tool('mousemove','--window',window,'332','415')
                    time.sleep(.2)
                    tool('mouseup','1')
                    time.sleep(.2)
                if mode in ('shortcut','shortcutmeta','shortcutdefault'):
                    tool('key','Return')
                    time.sleep(.2)
                    assert process.poll() is None,'Disabled default accept shortcut still triggered'
                    if mode=='shortcutmeta':
                        tool('key','F8')
                        time.sleep(.2)
                        assert process.poll() is None,'Meta binding triggered without modifier'
                        tool('key','Super_L+F8')
                    else: tool('key','ctrl+s' if mode=='shortcutdefault' else 'F8')
                if mode=='openwith':
                    tool('key','ctrl+o')
                    time.sleep(.4)
                    tool('key','Escape')
                    time.sleep(.2)
                    tool('key','Return')
                if mode == 'fixed': tool('key','ctrl+s')
                if mode in ('save-retry','save-cancel'):
                    tool('key','ctrl+s')
                    time.sleep(.4)
                    if mode == 'save-cancel':
                        tool('key','Escape')
                        time.sleep(.2)
                        tool('key','Return')
                    else:
                        tool('key','ctrl+a')
                        time.sleep(.2)
                        tool('type','--clearmodifiers',str(Path(directory)/'missing'/'bad.png'))
                        tool('key','Return')
                        time.sleep(.4)
                        assert process.poll() is None,'Save failure discarded the editor'
                        tool('key','ctrl+a')
                        time.sleep(.2)
                        tool('type','--clearmodifiers',str(Path(directory)/'retry.png'))
                        tool('key','Return')

                if mode=='uploadcancel':
                    tool('key','F11')
                    time.sleep(.4)
                    tool('key','Escape')
                    time.sleep(.2)
                    assert process.poll() is None,'Cancelling upload confirmation closed capture'
                    tool('key','Return')
                if mode in ('pixelatesafe','pixelateinsecure'):
                    for key in ('r','b'):
                        tool('key',key)
                        time.sleep(.2)
                        tool('mousemove','--window',window,'130','220')
                        tool('mousedown','1')
                        tool('mousemove','--window',window,'190','280')
                        time.sleep(.3)
                        if key=='b':
                            import io
                            from PIL import Image
                            result=subprocess.run(bus_command+[str(binary),'full','--raw'],env=env,capture_output=True,timeout=20)
                            assert result.returncode==0,result.stderr.decode()
                            preview=Image.open(io.BytesIO(result.stdout)).convert('RGB').getpixel((160,250))
                            assert preview==((255,0,0) if mode=='pixelateinsecure' else (255,255,255)),('Wrong pixelation preview',preview)
                        tool('mouseup','1')
                        time.sleep(.2)
                    tool('key','Return')
                if mode=='buttons':
                    captured=subprocess.run(bus_command+[str(binary),'full','--raw'],env=env,capture_output=True,timeout=20)
                    assert captured.returncode==0,captured.stderr.decode()
                    (artifact_dir/'rust-toolbar-x11.png').write_bytes(captured.stdout)
                    import io,configparser
                    from PIL import Image
                    pixels=Image.open(io.BytesIO(captured.stdout)).convert('RGB')
                    background=pixels.getpixel((950,38))
                    edge=max(x for x in range(20,900) if pixels.getpixel((x,38))!=background)
                    assert edge<550,'Hidden tools still expanded the toolbar'
                    for x,expected in ((edge-49,4),(edge-18,3)):
                        tool('mousemove','--window',window,str(x),'38')
                        tool('click','1')
                        time.sleep(.3)
                        saved=configparser.ConfigParser();saved.read(test_config)
                        assert int(saved.get('General','drawThickness').strip(chr(34)))==expected,'Toolbar size button did not apply'
                    tool('key','Return')
                if mode=='palette':
                    tool('mousemove','--window',window,'600','600')
                    tool('click','3')
                    time.sleep(.3)
                    tool('mousemove','--window',window,'615','615')
                    tool('click','1')
                    time.sleep(.3)
                    assert '#00ff00ff' in test_config.read_text(),test_config.read_text()
                    tool('key','r')
                    time.sleep(.2)
                    tool('mousemove','--window',window,'130','220')
                    tool('mousedown','1')
                    tool('mousemove','--window',window,'160','250')
                    time.sleep(.2)
                    tool('mouseup','1')
                    time.sleep(.2)
                    tool('key','Return')
                if mode in ('undolimit','color'):
                    tool('key','r')
                    time.sleep(.2)
                    for x,y in ((130,220),(190,280)):
                        tool('mousemove','--window',window,str(x),str(y))
                        tool('mousedown','1')
                        tool('mousemove','--window',window,str(x+30),str(y+30))
                        time.sleep(.2)
                        tool('mouseup','1')
                        time.sleep(.2)
                    for _ in range(2 if mode=='undolimit' else 0):
                        tool('key','ctrl+z')
                        time.sleep(.2)
                    tool('key','Return')
                if mode=='toolsize':
                    for tool_key in ('m','p','b','F10','r','t'):
                        tool('key',tool_key)
                        time.sleep(.15)
                        tool('key','F9')
                        time.sleep(.15)
                    tool('mousemove','--window',window,'600','600')
                    tool('key','m')
                    time.sleep(.2)
                    for button,expected_size in (('4',10),('4',11),('5',10)):
                        tool('click',button)
                        time.sleep(.3)
                        import configparser
                        wheel_saved=configparser.ConfigParser()
                        wheel_saved.read(test_config)
                        assert int(wheel_saved.get('General','drawMarkerSize').strip(chr(34)))==expected_size,(button,expected_size,test_config.read_text())
                    tool('key','F10')
                    time.sleep(.2)
                    tool('keydown','Control_L')
                    tool('click','4')
                    tool('keyup','Control_L')
                    time.sleep(.2)
                    import configparser
                    saved=configparser.ConfigParser()
                    saved.read(test_config)
                    for key,expected in [('drawThickness',4),('drawMarkerSize',10),('drawPixelateSize',3),('drawCircleCounterSize',2),('drawRectangleSize',5),('drawFontSize',13),('undoLimit',1)]:
                        assert int(saved.get('General',key).strip(chr(34)))==expected,(key,test_config.read_text())
                    tool('key','p')
                    time.sleep(.15)
                    tool('key','Return')
                if mode=='opacity':
                    time.sleep(.3)
                    captured=subprocess.run(bus_command+[str(binary),'full','--raw'],env=env,capture_output=True,timeout=20)
                    assert captured.returncode==0,captured.stderr.decode()
                    import io
                    from PIL import Image
                    pixels=Image.open(io.BytesIO(captured.stdout)).convert('RGB')
                    assert pixels.getpixel((500,500))==(0,0,0),'Configured opaque shade was ignored'
                    assert pixels.getpixel((200,180))==(0,255,255),('Configured selection color was ignored',pixels.getpixel((200,180)))
                    assert pixels.getpixel((250,350))==(255,255,255),('Shade leaked into selection',pixels.getpixel((250,350)))
                    tool('key','Return')
                if mode in ('raw','adjust','magnifier'): tool('key', 'Return')
            out, err = process.communicate(timeout=15)
            assert process.returncode == (2 if cancel else 0), err.decode()
            if cancel:
                assert out == b''
            elif mode in ('monitors','monitoroverride','monitordelay'):
                import io
                from PIL import Image
                pixels=Image.open(io.BytesIO(out)).convert('RGB')
                assert pixels.size==(512,768),pixels.size
                assert pixels.getpixel((256,384))==((0,0,255) if mode=='monitoroverride' else (255,0,0)),'Wrong X11 monitor captured'
            elif mode in ('textedit','objectmove'):
                assert png(out)==(400,200)
            elif mode in ('preset','last','lastconfig','lastoverride'):
                assert out==(b'20x10+5+6\n' if mode=='lastoverride' else b'80x60+20+30\n'),out
                cache=Path(directory)/'flameshot/flameshot/region.txt'
                assert cache.read_bytes()==(struct.pack('>iiii',5,6,24,15) if mode=='lastoverride' else struct.pack('>iiii',20,30,99,89))
            elif mode == 'adjust':
                assert out == b'211x205+121+210\n', out
            elif mode == 'geometry':
                assert out == b'200x200+100+180\n', out
            else:
                if mode == 'autosave':
                    assert not out
                    out = (Path(directory)/'auto.png').read_bytes()
                width, height = png(out)
                assert 199 <= width <= 201 and 199 <= height <= 201, (width, height)
                if mode in ('pixelatesafe','pixelateinsecure'):
                    import io
                    from PIL import Image
                    assert Image.open(io.BytesIO(out)).convert('RGB').getpixel((60,70))==preview,'Pixelation export differs from preview'
                if mode=='geometryoverlay':
                    import io
                    from PIL import Image
                    assert set(Image.open(io.BytesIO(out)).convert('RGB').getdata())=={(255,255,255)},'Geometry overlay leaked into saved image'
                if mode=='palette':
                    import io
                    from PIL import Image
                    assert Image.open(io.BytesIO(out)).convert('RGB').getpixel((40,50))==(0,255,0),'Palette selection did not affect exported drawing'
                if mode=='color':
                    import io
                    from PIL import Image
                    pixels=Image.open(io.BytesIO(out)).convert('RGBA')
                    actual=pixels.getpixel((40,50))
                    assert actual[0]==0 and abs(actual[1]-128)<=1 and actual[2]==0 and actual[3]==255,('Straight RGBA was applied more than once',actual)
                if mode=='undolimit':
                    import io
                    from PIL import Image
                    pixels=Image.open(io.BytesIO(out)).convert('RGB')
                    assert pixels.getpixel((40,50))==(255,0,0),'Undo exceeded the configured history limit'
                    assert pixels.getpixel((100,110))!=(255,0,0),'Latest drawing was not undone'
                if mode == 'fixed': assert png((Path(directory)/'fixed.png').read_bytes())==(200,200)
                if mode == 'save-retry':
                    assert (Path(directory)/'retry.png').exists(), (err.decode(),list(Path(directory).rglob('*')))
                    assert png((Path(directory)/'retry.png').read_bytes()) == (200,200)
                    assert not (Path(directory)/'retry_1.png').exists(),'Duplicate save after editor closed'
                print(f'GUI selection exported {width}x{height} PNG')
            process = None
        print('PASS: save retry/cancel, geometry/raw priority, auto-accept/save, X11 full/screen capture, invalid screen rejection, visible editor, Escape cancellation, drag selection and Enter export')
finally:
    if process is not None and process.poll() is None:
        process.terminate()
        process.wait(timeout=5)
    xvfb.terminate()
    try: xvfb.wait(timeout=5)
    except subprocess.TimeoutExpired: xvfb.kill(); xvfb.wait()
    xvfb_log.close()
