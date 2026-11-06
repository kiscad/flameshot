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
env = os.environ.copy()
env.pop('WAYLAND_DISPLAY', None)
env['XDG_SESSION_TYPE'] = 'x11'
env['LD_LIBRARY_PATH'] = str(root / 'usr/lib/x86_64-linux-gnu') + ':' + env.get('LD_LIBRARY_PATH', '')
xvfb = subprocess.Popen([str(root / 'usr/bin/Xvfb'), '-displayfd', '1', '-screen', '0', '1024x768x24', '-nolisten', 'tcp'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
process = None
try:
    display = xvfb.stdout.readline().decode().strip()
    assert display, xvfb.stderr.read().decode()
    env['DISPLAY'] = ':' + display
    with tempfile.TemporaryDirectory(prefix='flameshot-desktop-') as directory:
        bus_config = Path(directory) / 'bus.conf'
        bus_config.write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
        bus_command = ['dbus-run-session', '--config-file', str(bus_config), '--']
        env['XDG_CONFIG_HOME'] = directory
        env['XDG_CACHE_HOME'] = directory
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
        invalid = subprocess.run(bus_command + [str(binary), 'screen', '--number', '999', '--raw'], env=env, capture_output=True, timeout=10)
        assert invalid.returncode == 1 and invalid.stdout == b''
        assert b'out of range' in invalid.stderr
        clipboard_check = r"""
import subprocess,sys,struct,time
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
editor=subprocess.Popen([binary,'gui'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
try:
    window=wait_window('^Flameshot$')
    tool('windowsize','--sync',window,'1024','768')
    tool('windowfocus','--sync',window)
    time.sleep(.4)
    tool('mousemove','--window',window,'100','180')
    tool('mousedown','1')
    tool('mousemove','--window',window,'300','380')
    time.sleep(.2)
    tool('mouseup','1')
    time.sleep(.2)
    tool('key','ctrl+c')
    out,err=editor.communicate(timeout=10)
    assert editor.returncode==0,err.decode()
    assert not out
    copied=subprocess.run([xclip,'-selection','clipboard','-t','image/png','-o'],capture_output=True,timeout=5)
    assert copied.returncode==0,copied.stderr.decode()
    assert struct.unpack('>II',copied.stdout[16:24])==(200,200)
finally:
    if editor.poll() is None:editor.kill();editor.wait()
print('PASS: editor Ctrl+C preserves cropped clipboard after closing')
pinned=subprocess.run([binary,'screen','--number','0','--pin'],capture_output=True,timeout=20)
assert pinned.returncode==0,pinned.stderr.decode()
window=wait_window('^flameshot-pin$')
tool('windowfocus','--sync',window)
time.sleep(.3)
tool('key','Escape')
time.sleep(.3)
assert tool('search','--onlyvisible','--name','^flameshot-pin$').returncode!=0
print('PASS: D-Bus pin window displays and closes')
"""
        copied = subprocess.run(bus_command + [sys.executable, '-c', clipboard_check, str(binary), str(root / 'usr/bin/xclip'), str(root / 'usr/bin/xdotool')], env=env, capture_output=True, timeout=30)
        assert copied.returncode == 0, copied.stderr.decode()
        print(copied.stdout.decode().strip())
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
        assert not (Path(directory)/'flameshot/flameshot.ini').exists()
        process=None
        print('PASS: configuration window opens and closes without modifying settings')
        for mode in ('cancel','raw','geometry','both','autosave','adjust','magnifier','save-retry','save-cancel'):
            cancel = mode == 'cancel'
            flags = ['--raw'] if mode in ('cancel','raw') else ['--accept-on-select','--print-geometry']
            if mode == 'both': flags += ['--raw']
            if mode == 'adjust': flags = ['--print-geometry']
            if mode in ('save-retry','save-cancel'): flags = ['--raw']
            if mode == 'magnifier':
                flags = ['--raw']
                config = Path(directory)/'flameshot/flameshot.ini'
                config.parent.mkdir(exist_ok=True)
                config.write_text('[General]\nshowMagnifier=true\nuiLanguage=en\n')
            if mode == 'autosave': flags = ['--accept-on-select','--path',str(Path(directory)/'auto.png')]
            process = subprocess.Popen(bus_command + [str(binary), 'gui', *flags], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
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
            # Xvfb has no window manager to honor fullscreen requests.
            assert tool('windowsize', '--sync', window, '1024', '768').returncode == 0
            assert tool('windowfocus', '--sync', window).returncode == 0
            time.sleep(0.5)
            if mode == 'magnifier':
                tool('mousemove','--window',window,'400','500')
                time.sleep(.3)
                artifact = subprocess.run(bus_command+[str(binary),'full','--raw'],env=env,capture_output=True,timeout=20)
                assert artifact.returncode==0,artifact.stderr.decode()
                (binary.parent.parent/'rust-magnifier-x11.png').write_bytes(artifact.stdout)
                tool('key','g')
            if cancel:
                tool('key', 'Escape')
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

                if mode in ('raw','adjust','magnifier'): tool('key', 'Return')
            out, err = process.communicate(timeout=15)
            assert process.returncode == (2 if cancel else 0), err.decode()
            if cancel:
                assert out == b''
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
    xvfb.wait(timeout=5)
