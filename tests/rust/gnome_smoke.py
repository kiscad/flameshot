#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Private headless GNOME Wayland fixture. Usage: gnome_smoke.py FLAMESHOT_BINARY
Requires GNOME Shell, GNOME/GTK portals, PyGObject and their normal runtime libraries.
Debug input/evaluation is enabled only in the disposable private compositor.
"""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

binary=Path(sys.argv[1]).resolve()
if '--inside' not in sys.argv:
    with tempfile.TemporaryDirectory(prefix='flameshot-gnome-') as directory:
        root=Path(directory)
        for folder in ['config','data','cache','runtime','tmp','pictures']:(root/folder).mkdir(mode=0o700)
        extension=root/'data/gnome-shell/extensions/flameshot-test@local'
        extension.mkdir(parents=True)
        version=subprocess.run(['gnome-shell','--version'],capture_output=True,text=True,check=True).stdout.split()[-1].split('.')[0]
        (extension/'metadata.json').write_text(json.dumps({'uuid':'flameshot-test@local','name':'Isolated test input','description':'Private Flameshot acceptance fixture','shell-version':[version]}))
        (extension/'extension.js').write_text('''import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import St from 'gi://St';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
export default class TestInput extends Extension {
 enable() {
  global.context.unsafe_mode = true;
  const seat = Clutter.get_default_backend().get_default_seat();
  const keyboard = seat.create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
  const pointer = seat.create_virtual_device(Clutter.InputDeviceType.POINTER_DEVICE);
  global.flameshotTest = {
   key: (value, pressed) => keyboard.notify_keyval(GLib.get_monotonic_time(), value, pressed ? Clutter.KeyState.PRESSED : Clutter.KeyState.RELEASED),
   move: (x,y) => pointer.notify_absolute_motion(GLib.get_monotonic_time(),x,y),
   button: (button, pressed) => pointer.notify_button(GLib.get_monotonic_time(),button,pressed ? Clutter.ButtonState.PRESSED : Clutter.ButtonState.RELEASED),
   hideOverview: () => Main.overview.hide(),
   paint: () => { const background = new St.Widget({style:'background-color: rgb(32,64,96);',width:1024,height:768}); Main.layoutManager._backgroundGroup.add_child(background); global.flameshotTest.background=background; }

  };
 }
 disable() { delete global.flameshotTest; global.context.unsafe_mode = false; }
}
''')
        settings=root/'config/glib-2.0/settings';settings.mkdir(parents=True)
        (settings/'keyfile').write_text("[org/gnome/shell]\nenabled-extensions=['flameshot-test@local']\ndisable-user-extensions=false\nwelcome-dialog-last-shown-version='99.0'\n[org/gnome/desktop/interface]\nenable-animations=false\n")
        (root/'config/user-dirs.dirs').write_text(f'XDG_PICTURES_DIR="{root}/pictures"\n')
        config=root/'config/flameshot';config.mkdir()
        (config/'flameshot.ini').write_text('[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\n')
        portals=root/'config/xdg-desktop-portal';portals.mkdir()
        (portals/'portals.conf').write_text('[preferred]\ndefault=gnome;gtk;\n')
        bus=root/'bus.conf';bus.write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
        env=os.environ.copy()
        for key in ['DISPLAY','WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS','DBUS_SYSTEM_BUS_ADDRESS','SESSION_MANAGER','IBUS_ADDRESS','GNOME_DESKTOP_SESSION_ID']:env.pop(key,None)
        env.update(TMPDIR=str(root/'tmp'),XDG_RUNTIME_DIR=str(root/'runtime'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),XDG_CURRENT_DESKTOP='GNOME',XDG_SESSION_TYPE='wayland',GNOME_SHELL_SESSION_MODE='user',GSETTINGS_BACKEND='keyfile',LIBGL_ALWAYS_SOFTWARE='1',NO_AT_BRIDGE='1',LC_ALL='C.UTF-8')
        result=subprocess.run(['dbus-run-session','--config-file',str(bus),'--',sys.executable,__file__,str(binary),'--inside',str(root)],env=env,timeout=100)
        if result.returncode:
            for log in root.glob('*.log'):print(log.name,log.read_text(errors='replace')[-7000:],file=sys.stderr)
        sys.exit(result.returncode)

from gi.repository import Gio,GLib
root=Path(sys.argv[-1]);env=os.environ.copy()
# No call from the private compositor reaches the host's system services.
env['DBUS_SYSTEM_BUS_ADDRESS']=env['DBUS_SESSION_BUS_ADDRESS']
env['WAYLAND_DISPLAY']='flameshot-test'
processes=[];logs=[]
def start(name,args):
    log=open(root/(name+'.log'),'w');logs.append(log)
    process=subprocess.Popen(args,env=env,stdout=log,stderr=log,start_new_session=True)
    processes.append(process)
    return process
connection=Gio.bus_get_sync(Gio.BusType.SESSION,None)
def call(destination,path,method,signature,args,timeout=3000):
    interface,member=method.rsplit('.',1)
    return connection.call_sync(destination,path,interface,member,GLib.Variant(signature,args),None,Gio.DBusCallFlags.NONE,timeout,None).unpack()
def evaluate(code):
    success,result=call('org.gnome.Shell','/org/gnome/Shell','org.gnome.Shell.Eval','(s)',(code,))
    assert success,result
    return json.loads(result) if result else None
def click_shell_button(label):
    position=evaluate("(()=>{function walk(a){if(a.mapped&&a.get_label&&a.get_label()==="+json.dumps(label)+"){let [x,y]=a.get_transformed_position();let [w,h]=a.get_transformed_size();return [x+w/2,y+h/2]}for(let c of a.get_children()){let found=walk(c);if(found)return found}return null}return walk(global.stage)})()")
    if position:
        evaluate(f'global.flameshotTest.move({position[0]},{position[1]})')
        evaluate('global.flameshotTest.button(1,true)');time.sleep(.05)
        evaluate('global.flameshotTest.button(1,false)')
    return position is not None
def key(value):
    evaluate(f'global.flameshotTest.key({value},true)');time.sleep(.05)
    evaluate(f'global.flameshotTest.key({value},false)')
try:
    shell=start('shell',['gnome-shell','--headless','--wayland','--virtual-monitor','1024x768','--wayland-display','flameshot-test','--no-x11','--sm-disable'])
    deadline=time.monotonic()+15
    while True:
        try:
            if evaluate('Boolean(global.flameshotTest)'):break
        except (GLib.Error,AssertionError):pass
        assert shell.poll() is None and time.monotonic()<deadline,'Private GNOME failed to initialize'
        time.sleep(.1)
    deadline=time.monotonic()+10
    while evaluate('Main.layoutManager._startingUp'):
        assert time.monotonic()<deadline,'GNOME startup did not finish'
        time.sleep(.1)
    evaluate('global.flameshotTest.hideOverview()')
    while evaluate('Main.overview.visible'):
        assert time.monotonic()<deadline,'GNOME overview did not hide'
        time.sleep(.1)
    print('PASS: private GNOME compositor and virtual input initialized',flush=True)
    start('permissions',['/usr/libexec/xdg-permission-store'])
    start('portal-gnome',['/usr/libexec/xdg-desktop-portal-gnome'])
    start('portal-gtk',['/usr/libexec/xdg-desktop-portal-gtk'])
    deadline=time.monotonic()+10
    for service in ['org.freedesktop.impl.portal.desktop.gnome','org.freedesktop.impl.portal.desktop.gtk']:
        while not call('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus.NameHasOwner','(s)',(service,))[0]:
            assert time.monotonic()<deadline,'Backend unavailable: '+service
            time.sleep(.1)
    start('portal',['/usr/libexec/xdg-desktop-portal'])
    deadline=time.monotonic()+10
    while True:
        try:
            call('org.freedesktop.portal.Desktop','/org/freedesktop/portal/desktop','org.freedesktop.DBus.Properties.Get','(ss)',('org.freedesktop.portal.Screenshot','version'));break
        except GLib.Error:
            assert time.monotonic()<deadline,'Screenshot portal unavailable';time.sleep(.1)
    evaluate('global.flameshotTest.paint()')
    time.sleep(.3)
    capture=subprocess.Popen([str(binary),'full','--raw'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
    processes.append(capture)
    deadline=time.monotonic()+12
    while capture.poll() is None and time.monotonic()<deadline:
        windows=evaluate('global.get_window_actors().map(a=>({title:a.meta_window.get_title(),pid:a.meta_window.get_pid(),rect:a.meta_window.get_frame_rect()}))')
        click_shell_button('Allow')
        if windows:
            evaluate('global.get_window_actors().at(-1).meta_window.activate(global.get_current_time())')
            key(65293)
        time.sleep(.5)
    out,err=capture.communicate(timeout=10)
    assert capture.returncode==0,err.decode()
    import struct,io
    from PIL import Image
    artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
    artifacts.mkdir(parents=True,exist_ok=True)
    (artifacts/'rust-gnome-full.png').write_bytes(out)
    assert Image.open(io.BytesIO(out)).getpixel((200,300))[:3]==(32,64,96)
    assert out[:8]==b'\x89PNG\r\n\x1a\n'
    assert struct.unpack('>II',out[16:24])==(1024,768)
    print('PASS: native GNOME screenshot portal exports 1024x768 PNG',flush=True)
    screen=subprocess.run([str(binary),'screen','--number','0','--raw'],env=env,capture_output=True,timeout=20)
    assert screen.returncode==0,screen.stderr.decode()
    assert screen.stdout[:8]==b'\x89PNG\r\n\x1a\n'
    assert struct.unpack('>II',screen.stdout[16:24])==(1024,768)
    for accept in [True,False]:
        args=['gui','--raw']+(['--region','80x60+200+300'] if accept else [])
        editor=subprocess.Popen([str(binary),*args],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
        processes.append(editor)
        deadline=time.monotonic()+12
        while True:
            windows=evaluate("global.get_window_actors().filter(a=>a.meta_window.get_title()==='Flameshot'&&a.mapped).map(a=>a.meta_window.get_client_type()===Meta.WindowClientType.WAYLAND)")
            if windows:
                assert windows==[True],windows
                break
            if editor.poll() is not None:
                out,err=editor.communicate()
                raise AssertionError('Editor exited: '+err.decode())
            if time.monotonic()>=deadline:
                print('existing windows',evaluate('({overview:Main.overview.visible,starting:Main.layoutManager._startingUp,windows:global.get_window_actors().map(a=>({title:a.meta_window.get_title(),mapped:a.mapped,hidden:a.meta_window.is_hidden(),minimized:a.meta_window.minimized}))})'),flush=True)
                raise AssertionError('Native editor did not map')
            time.sleep(.1)
        evaluate("global.get_window_actors().find(a=>a.meta_window.get_title()==='Flameshot').meta_window.activate(global.get_current_time())")
        time.sleep(.3)
        key(65293 if accept else 65307)
        out,err=editor.communicate(timeout=10)
        assert editor.returncode==(0 if accept else 2),err.decode()
        if accept:
            assert out[:8]==b'\x89PNG\r\n\x1a\n'
            assert struct.unpack('>II',out[16:24])==(80,60)
            assert Image.open(io.BytesIO(out)).getpixel((40,30))[:3]==(32,64,96)
            (artifacts/'rust-gnome-region.png').write_bytes(out)
        else:assert not out
    print('PASS: GNOME screen capture, native Wayland editor export and Escape cancellation',flush=True)
    call('org.freedesktop.impl.portal.PermissionStore','/org/freedesktop/impl/portal/PermissionStore','org.freedesktop.impl.portal.PermissionStore.Delete','(ss)',('screenshot','screenshot'))
    denied=subprocess.Popen([str(binary),'full','--raw'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
    processes.append(denied)
    deadline=time.monotonic()+10
    clicked=False
    while denied.poll() is None and time.monotonic()<deadline:
        clicked=click_shell_button('Deny') or clicked
        time.sleep(.1)
    out,err=denied.communicate(timeout=5)
    assert clicked and denied.returncode==1 and not out,(denied.returncode,err.decode())
    assert b'rejected' in err
    print('PASS: real GNOME permission refusal returns status 1 without PNG output',flush=True)


finally:
    for process in reversed(processes):
        if process.poll() is not None:continue
        try:os.killpg(process.pid,signal.SIGTERM)
        except ProcessLookupError:pass
    for process in reversed(processes):
        try:process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid,signal.SIGKILL);process.wait()
    for log in logs:log.close()
