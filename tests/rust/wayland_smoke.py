#!/usr/bin/env python3
"""Isolated native Wayland/Sway smoke test. No XWayland or real desktop bus.
Usage: python3 tests/rust/wayland_smoke.py TOOL_ROOT FLAMESHOT_BINARY [--ime]
TOOL_ROOT supplies sway, swaymsg, wtype, wl-paste, grim, portal-wlr and its libraries.
Uses the host's xdg-desktop-portal frontend and pipewire executable.
Build the pointer helper first: cargo build -p flameshot-platform --example wayland_pointer
"""
import shutil
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import time

root,binary=map(lambda p:Path(p).resolve(),sys.argv[1:3])
artifact_dir=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifact_dir.mkdir(parents=True,exist_ok=True)
helper_dir=Path(os.environ.get('FLAMESHOT_TEST_HELPER_DIR',str(binary.parent/'examples')))
if '--inside' not in sys.argv:
    with tempfile.TemporaryDirectory(prefix='flameshot-wayland-') as directory:
        directory=Path(directory)
        runtime=directory/'runtime';runtime.mkdir(mode=0o700)
        config=directory/'config';config.mkdir()
        (config/'flameshot').mkdir();(config/'flameshot/flameshot.ini').write_text('[General]\ncheckForUpdates=false\nuseX11LegacyScreenshot=true\n')
        (config/'xdg-desktop-portal').mkdir();(config/'xdg-desktop-portal/portals.conf').write_text('[preferred]\ndefault=wlr\n')
        portals=directory/'portals';portals.mkdir()
        shutil.copy(root/'usr/share/xdg-desktop-portal/portals/wlr.portal',portals)
        shutil.copy('/usr/share/xdg-desktop-portal/portals/gtk.portal',portals)
        (portals/'portals.conf').write_text('[preferred]\ndefault=gtk\norg.freedesktop.impl.portal.Screenshot=wlr\norg.freedesktop.impl.portal.ScreenCast=wlr\n')
        (directory/'sway.conf').write_text('xwayland disable\noutput HEADLESS-1 resolution 1024x768\noutput * bg #204060 solid_color\nseat seat0 fallback true\n')
        bus=directory/'bus.conf';bus.write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><policy context="default"><allow send_destination="*"/><allow send_type="error" send_requested_reply="false"/><allow receive_type="error" receive_requested_reply="false"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
        env=os.environ.copy()
        for key in ['DISPLAY','WAYLAND_DISPLAY','SWAYSOCK','DBUS_SESSION_BUS_ADDRESS','FLAMESHOT_TEST_WAYLAND_IME']:env.pop(key,None)
        env.update(XDG_RUNTIME_DIR=str(runtime),XDG_CONFIG_HOME=str(config),XDG_CACHE_HOME=str(directory/'cache'),XDG_DATA_HOME=str(directory/'data'),XDG_SESSION_TYPE='wayland',XDG_CURRENT_DESKTOP='sway',WLR_BACKENDS='headless',WLR_RENDERER='pixman',WLR_LIBINPUT_NO_DEVICES='1',WLR_HEADLESS_OUTPUTS='1',XDG_DESKTOP_PORTAL_DIR=str(portals))
        env['LD_LIBRARY_PATH']=str(root/'usr/lib/x86_64-linux-gnu')+':'+env.get('LD_LIBRARY_PATH','')
        env['PATH']=str(root/'usr/bin')+':'+env['PATH']
        if '--ime' in sys.argv:env['FLAMESHOT_TEST_WAYLAND_IME']='1'
        result=subprocess.run(['dbus-run-session','--config-file',str(bus),'--',sys.executable,__file__,str(root),str(binary),'--inside',str(directory)],env=env,timeout=120)
        if result.returncode:
            for log in directory.glob('*.log'):print(log.name,log.read_text(errors='replace')[-5000:],file=sys.stderr)
        sys.exit(result.returncode)

directory=Path(sys.argv[-1]);env=os.environ.copy();processes=[];logs=[]
def start(name,args):
    log=(directory/(name+'.log')).open('w');logs.append(log)
    process=subprocess.Popen(args,env=env,stdout=log,stderr=log);processes.append(process);return process
def wait_for(predicate,timeout=10):
    deadline=time.monotonic()+timeout
    while time.monotonic()<deadline:
        if predicate():return
        time.sleep(.1)
    raise AssertionError('Timed out waiting for isolated desktop service')
def png(data):
    assert data[:8]==b'\x89PNG\r\n\x1a\n',data[:200]
    return struct.unpack('>II',data[16:24])
def call(*args,timeout=25):return subprocess.run(args,env=env,capture_output=True,timeout=timeout)
try:
    sway=start('sway',[str(root/'usr/bin/sway'),'--unsupported-gpu','-c',str(directory/'sway.conf')])
    wait_for(lambda:any((directory/'runtime').glob('wayland-*')))
    sockets=[p for p in (directory/'runtime').glob('wayland-*') if not p.name.endswith('.lock')]
    env['WAYLAND_DISPLAY']=sockets[0].name
    wait_for(lambda:any((directory/'runtime').glob('sway-ipc.*.sock')))
    env['SWAYSOCK']=str(next((directory/'runtime').glob('sway-ipc.*.sock')))
    pointer_env=env.copy();pointer_env['FLAMESHOT_TEST_WAYLAND_INPUT']='1'
    pointer=subprocess.Popen([str(helper_dir/'wayland_pointer')],env=pointer_env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
    processes.append(pointer);assert pointer.stdout.readline().strip()=='ready'
    def point(command):
        pointer.stdin.write(command+'\n');pointer.stdin.flush();assert pointer.stdout.readline().strip()=='ok'
    start('pipewire',['/usr/bin/pipewire'])
    wait_for(lambda:(directory/'runtime/pipewire-0').exists())
    start('permission-store',['/usr/libexec/xdg-permission-store'])
    start('portal-gtk',['/usr/libexec/xdg-desktop-portal-gtk'])
    start('portal-wlr',[str(root/'usr/libexec/xdg-desktop-portal-wlr')])
    def owned(name):
        result=call('gdbus','call','--session','--dest','org.freedesktop.DBus','--object-path','/org/freedesktop/DBus','--method','org.freedesktop.DBus.NameHasOwner',name,timeout=5)
        return result.returncode==0 and b'true' in result.stdout
    for name in ['org.freedesktop.impl.portal.PermissionStore','org.freedesktop.impl.portal.desktop.gtk','org.freedesktop.impl.portal.desktop.wlr']:wait_for(lambda name=name:owned(name))
    start('portal',['/usr/libexec/xdg-desktop-portal','--verbose'])
    def portal_ready():
        return call('gdbus','introspect','--session','--dest','org.freedesktop.portal.Desktop','--object-path','/org/freedesktop/portal/desktop',timeout=5).returncode==0
    wait_for(portal_ready)
    permission=call('gdbus','call','--session','--dest','org.freedesktop.impl.portal.PermissionStore','--object-path','/org/freedesktop/impl/portal/PermissionStore','--method','org.freedesktop.impl.portal.PermissionStore.SetPermission','screenshot','true','screenshot','',"['yes']")
    assert permission.returncode==0,permission.stderr.decode()
    result=call(str(binary),'full','--raw');assert result.returncode==0,result.stderr.decode();assert png(result.stdout)==(1024,768)
    result=call(str(binary),'screen','--number','0','--raw');assert result.returncode==0,result.stderr.decode();assert png(result.stdout)==(1024,768)
    result=call(str(binary),'full','--clipboard');assert result.returncode==0,result.stderr.decode()
    pasted=call(str(root/'usr/bin/wl-paste'),'--type','image/png');assert pasted.returncode==0,pasted.stderr.decode();assert png(pasted.stdout)==(1024,768)
    print('PASS: native Wayland full/screen capture and clipboard persistence with DISPLAY unset',flush=True)
    clipboard_config=directory/'config/flameshot/flameshot.ini'
    clipboard_original=clipboard_config.read_text()
    clipboard_config.write_text('[General]\ncheckForUpdates=false\nautoCloseIdleDaemon=true\nuseJpgForClipboard=true\ncopyPathAfterSave=true\nuseX11LegacyScreenshot=true\n')
    result=call(str(binary),'full','--clipboard');assert result.returncode==0,result.stderr.decode()
    jpeg=call(str(root/'usr/bin/wl-paste'),'--type','image/jpeg');assert jpeg.returncode==0 and jpeg.stdout[:2]==bytes([255,216]),jpeg.stderr.decode()
    from PIL import Image
    from io import BytesIO
    assert Image.open(BytesIO(jpeg.stdout)).size==(1024,768)
    time.sleep(1.2)
    assert owned('org.flameshot.Flameshot'),'Wayland daemon exited while owning clipboard'
    destination=directory/'clipboard-path.png'
    saved=call(str(binary),'full','--path',str(destination));assert saved.returncode==0,saved.stderr.decode()
    text=call(str(root/'usr/bin/wl-paste'),'--type','text/plain;charset=utf-8','--no-newline')
    assert text.returncode==0 and text.stdout.decode()==str(destination),text.stderr.decode()
    time.sleep(1.2)
    assert owned('org.flameshot.Flameshot'),'Wayland daemon exited while owning replacement text'

    subprocess.run([str(root/'usr/bin/wl-copy'),'--type','text/plain','replacement'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=True,timeout=5)
    wait_for(lambda:not owned('org.flameshot.Flameshot'),8)
    clipboard_config.write_text(clipboard_original)
    print('PASS: native Wayland JPEG clipboard and ownership-aware idle exit',flush=True)
    if env.get('FLAMESHOT_TEST_WAYLAND_IME')=='1':
        profile=directory/'config/fcitx5';profile.mkdir()
        (profile/'profile').write_text('[Groups/0]\nName=Default\nDefault Layout=us\nDefaultIM=pinyin\n[Groups/0/Items/0]\nName=keyboard-us\nLayout=\n[Groups/0/Items/1]\nName=pinyin\nLayout=\n[GroupOrder]\n0=Default\n')
        fcitx=start('fcitx',['fcitx5','-D','--disable=cloudpinyin'])
        wait_for(lambda:call('fcitx5-remote','--check',timeout=5).returncode==0)
        editor=subprocess.Popen([str(binary),'gui','--raw','--region','400x200+500+450'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);processes.append(editor)
        def ime_editor_visible():
            tree=json.loads(call(str(root/'usr/bin/swaymsg'),'-t','get_tree').stdout)
            def contains(node):return (node.get('name')=='Flameshot' and node.get('pid')==editor.pid) or any(contains(c) for key in ['nodes','floating_nodes'] for c in node.get(key,[]))
            return contains(tree)
        wait_for(ime_editor_visible,20)
        call(str(root/'usr/bin/swaymsg'),'[title="Flameshot"] focus')
        call(str(root/'usr/bin/wtype'),'-s','500','t')
        point('move 550 500');point('left');time.sleep(.5)
        assert call('fcitx5-remote','-s','pinyin').returncode==0
        call('fcitx5-remote','-o');time.sleep(.5)
        typed=call(str(root/'usr/bin/wtype'),'-d','100','nihao','-s','3000','-k','space','-s','500')
        assert typed.returncode==0,typed.stderr.decode()
        call('fcitx5-remote','-c');time.sleep(.3)
        call(str(root/'usr/bin/wtype'),'-M','ctrl','-k','a','-k','c','-m','ctrl','-s','300')
        copied=call(str(root/'usr/bin/wl-paste'),'--type','text/plain;charset=utf-8','--no-newline')
        assert copied.returncode==0 and copied.stdout.decode()=='你好',(copied.stdout,copied.stderr)
        call(str(root/'usr/bin/wtype'),'-k','End')
        call('fcitx5-remote','-o');time.sleep(.3)
        call(str(root/'usr/bin/wtype'),'-d','100','zhong','-s','500','-k','Escape','-s','300')
        assert editor.poll() is None,'Composition cancellation closed the editor'
        call('fcitx5-remote','-c');time.sleep(.3)
        call(str(root/'usr/bin/wtype'),'-M','ctrl','-k','a','-k','c','-m','ctrl','-s','300')
        copied=call(str(root/'usr/bin/wl-paste'),'--type','text/plain;charset=utf-8','--no-newline')
        assert copied.returncode==0 and copied.stdout.decode()=='你好','Composition cancellation changed committed text'
        call(str(root/'usr/bin/wtype'),'-M','ctrl','-k','Return','-m','ctrl','-s','300','-k','Return')
        out,err=editor.communicate(timeout=10);assert editor.returncode==0,err.decode()
        assert png(out)==(400,200)
        image=Image.open(BytesIO(out)).convert('RGB')
        assert any(r>180 and g<80 and b<80 for r,g,b in image.getdata()),'Missing text annotation pixels'
        (artifact_dir/'rust-wayland-ime.png').write_bytes(out)
        call('fcitx5-remote','-e');assert fcitx.wait(timeout=5)==0
        print('PASS: native Wayland Fcitx Pinyin commit, text copying and annotation export',flush=True)
    for copy in (False,True):
        flags=['--clipboard'] if copy else ['--raw']
        editor=subprocess.Popen([str(binary),'gui','--region','100x80+10+20',*flags],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);processes.append(editor)
        def editor_visible():
            result=call(str(root/'usr/bin/swaymsg'),'-t','get_tree',timeout=5)
            def contains(node):return (node.get('name')=='Flameshot' and node.get('pid')==editor.pid) or any(contains(c) for key in ['nodes','floating_nodes'] for c in node.get(key,[]))
            return result.returncode==0 and contains(json.loads(result.stdout))
        wait_for(editor_visible,20)
        focused=call(str(root/'usr/bin/swaymsg'),'[title="Flameshot"] focus');assert focused.returncode==0,focused.stderr.decode()
        keys=['-M','ctrl','-k','c','-m','ctrl'] if copy else ['-k','Return']
        result=call(str(root/'usr/bin/wtype'),'-s','500',*keys);assert result.returncode==0,result.stderr.decode()
        out,err=editor.communicate(timeout=15);assert editor.returncode==0,err.decode()
        if copy:
            assert not out
            pasted=call(str(root/'usr/bin/wl-paste'),'--type','image/png');assert pasted.returncode==0,pasted.stderr.decode();assert png(pasted.stdout)==(100,80)
        else:assert png(out)==(100,80)
    print('PASS: native Wayland editor exports and Ctrl+C clipboard survives window closure',flush=True)

    from PIL import Image
    from io import BytesIO
    image=Image.new('RGBA',(400,300),(255,0,0,255))
    image.paste((0,255,0,255),(200,0,400,300))
    buffer=BytesIO();image.save(buffer,format='PNG')
    message=struct.pack('>I',1)+buffer.getvalue()+struct.pack('>iiiid',100,100,499,399,1.0)
    pin_file=directory/'pin.dat';pin_file.write_bytes(message)
    with pin_file.open('rb') as source:
        pinned=subprocess.Popen([str(binary),'internal-pin'],stdin=source,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    processes.append(pinned)
    def pin_node():
        result=call(str(root/'usr/bin/swaymsg'),'-t','get_tree',timeout=5)
        def locate(node):
            if node.get('pid')==pinned.pid:return node
            for key in ['nodes','floating_nodes']:
                for child in node.get(key,[]):
                    found=locate(child)
                    if found:return found
        return locate(json.loads(result.stdout)) if result.returncode==0 else None
    wait_for(lambda:pin_node() is not None,20)
    for command in ['floating enable','border none','resize set width 400 px height 300 px','move position 100 100','focus']:
        result=call(str(root/'usr/bin/swaymsg'),f'[pid={pinned.pid}] {command}');assert result.returncode==0,result.stderr.decode()
    time.sleep(.3)
    def pin_pixels():
        result=call(str(root/'usr/bin/grim'),'-');assert result.returncode==0,result.stderr.decode()
        screenshot=Image.open(BytesIO(result.stdout)).convert('RGB');rect=pin_node()['rect']
        return [screenshot.getpixel((rect['x']+rect['width']*fraction//4,rect['y']+rect['height']//2)) for fraction in (1,3)]
    assert pin_pixels()==[(255,0,0),(0,255,0)],pin_pixels()
    call(str(root/'usr/bin/wtype'),'-s','500','5');time.sleep(.3)
    blended=pin_pixels();expected=[(144,32,48),(16,160,48)]
    assert all(abs(a-b)<=3 for pixel,target in zip(blended,expected) for a,b in zip(pixel,target)),blended
    call(str(root/'usr/bin/wtype'),'-s','200','0');time.sleep(.2)
    assert pin_pixels()==[(255,0,0),(0,255,0)],pin_pixels()
    rect=pin_node()['rect']
    point(f"move {rect['x']+20} {rect['y']+20}")
    time.sleep(.3)
    point('right')
    time.sleep(.3)
    snapshot=call(str(root/'usr/bin/grim'),'-c','-');(artifact_dir/'rust-wayland-pin-menu.png').write_bytes(snapshot.stdout)
    call(str(root/'usr/bin/wtype'),'-s','300','-k','Down','-s','150','-k','Down','-s','150','-k','Return')
    wait_for(lambda:pin_node()['rect']['width']==300 and pin_node()['rect']['height']==400,5)
    result=call(str(root/'usr/bin/grim'),'-');rotated=Image.open(BytesIO(result.stdout)).convert('RGB');rect=pin_node()['rect']
    vertical=[rotated.getpixel((rect['x']+rect['width']//2,rect['y']+rect['height']*fraction//4)) for fraction in (1,3)]
    assert vertical==[(255,0,0),(0,255,0)],vertical
    call(str(root/'usr/bin/wtype'),'-s','200','-k','Escape')
    out,err=pinned.communicate(timeout=10);assert pinned.returncode==0,err.decode()
    print('PASS: native Wayland pin opacity and rotation preserve expected composited pixels',flush=True)

    # Add a second output after the single-output checks to exercise hotplug,
    # a negative logical origin and a 2x/1x mixed-scale desktop.
    from PIL import Image
    from io import BytesIO
    for command in ['create_output','output HEADLESS-2 resolution 1280x960 scale 2 position -640 0','output HEADLESS-1 position 0 0','output HEADLESS-2 bg #e02020 solid_color']:
        result=call(str(root/'usr/bin/swaymsg'),command);assert result.returncode==0,result.stderr.decode()
    time.sleep(.5)
    full=call(str(binary),'full','--raw');assert full.returncode==0,full.stderr.decode()
    assert png(full.stdout)==(3328,1536),png(full.stdout)
    expected=[((2048,1536),(32,64,96)),((1280,960),(224,32,32))]
    for index,(size,color) in enumerate(expected):
        result=call(str(binary),'screen','--number',str(index),'--raw');assert result.returncode==0,result.stderr.decode()
        assert png(result.stdout)==size,(index,png(result.stdout))
        pixels=Image.open(BytesIO(result.stdout)).convert('RGB')
        assert pixels.getpixel((size[0]//2,size[1]//2))==color,(index,pixels.getpixel((size[0]//2,size[1]//2)))
    print('PASS: mixed-scale negative-origin monitor crops preserve dimensions and pixels',flush=True)
    capture_config=directory/'config/flameshot/flameshot.ini'
    capture_config.write_text('[General]\ncheckForUpdates=false\nuseX11LegacyScreenshot=true\ncaptureActiveMonitor=true\n')
    rejected=call(str(binary),'gui','--region','all','--raw',timeout=10)
    assert rejected.returncode==1 and not rejected.stdout and b'not supported on Wayland' in rejected.stderr,rejected.stderr.decode()
    print('PASS: active-monitor preference rejects unsupported Wayland pointer lookup',flush=True)
    for chooser in (False,True):
        if chooser: capture_config.write_text('[General]\ncheckForUpdates=false\nuseX11LegacyScreenshot=true\n')
        args=['gui'] if chooser else ['screen','--number','1','--edit']
        editor=subprocess.Popen([str(binary),*args,'--region','100x80+10+20','--raw'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);processes.append(editor)
        def editor_output():
            result=call(str(root/'usr/bin/swaymsg'),'-t','get_tree',timeout=5)
            def locate(node,output=None):
                if node.get('type')=='output':output=node.get('name')
                if node.get('pid')==editor.pid:
                    assert node.get('app_id')=='org.flameshot.Flameshot',node.get('app_id')
                    return output
                for key in ['nodes','floating_nodes']:
                    for child in node.get(key,[]):
                        found=locate(child,output)
                        if found:return found
            return locate(json.loads(result.stdout)) if result.returncode==0 else None
        wait_for(lambda:editor_output() is not None,20)
        call(str(root/'usr/bin/swaymsg'),f'[pid={editor.pid}] focus')
        if chooser:
            result=call(str(root/'usr/bin/wtype'),'-s','500','2');assert result.returncode==0,result.stderr.decode()
        wait_for(lambda:editor_output()=='HEADLESS-2',10)
        call(str(root/'usr/bin/swaymsg'),f'[pid={editor.pid}] focus')
        result=call(str(root/'usr/bin/wtype'),'-s','500','-k','Return');assert result.returncode==0,result.stderr.decode()
        out,err=editor.communicate(timeout=15);assert editor.returncode==0,err.decode()
        pixels=Image.open(BytesIO(out)).convert('RGB');assert pixels.size==(100,80)
        assert pixels.getpixel((50,40))==(224,32,32),pixels.getpixel((50,40))
    print('PASS: explicit and interactive monitor selection targets the correct scaled output',flush=True)

    capture_config.write_text('[General]\ncheckForUpdates=false\nshowQuitPrompt=true\nshowAbortNotification=false\n')
    closing=subprocess.Popen([str(binary),'gui','--region','screen1','--raw'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);processes.append(closing)
    def close_window_ready():
        result=call(str(root/'usr/bin/swaymsg'),'-t','get_tree',timeout=5)
        def contains(node):
            return node.get('pid')==closing.pid or any(contains(child) for key in ('nodes','floating_nodes') for child in node.get(key,[]))
        return result.returncode==0 and contains(json.loads(result.stdout))
    wait_for(close_window_ready,20)
    call(str(root/'usr/bin/swaymsg'),f'[pid={closing.pid}] focus')
    time.sleep(.3)
    call(str(root/'usr/bin/swaymsg'),f'[pid={closing.pid}] kill')
    time.sleep(.4)
    assert closing.poll() is None,'Native close bypassed quit confirmation'
    call(str(root/'usr/bin/swaymsg'),f'[pid={closing.pid}] focus')
    call(str(root/'usr/bin/wtype'),'-s','200','-k','Return','-s','200')
    time.sleep(.4)
    assert closing.poll() is None,'Default No accepted native close'
    snapshot=call(str(root/'usr/bin/grim'),'-o','HEADLESS-2','-');(artifact_dir/'rust-wayland-quit-after-no.png').write_bytes(snapshot.stdout)
    call(str(root/'usr/bin/swaymsg'),f'[pid={closing.pid}] focus')
    call(str(root/'usr/bin/wtype'),'-s','200','-k','Escape','-s','200')
    time.sleep(.3)
    call(str(root/'usr/bin/wtype'),'-s','200','-k','Tab','-s','200')
    time.sleep(.2)
    call(str(root/'usr/bin/wtype'),'-s','200','-k','Return','-s','200')
    time.sleep(.3)
    snapshot=call(str(root/'usr/bin/grim'),'-o','HEADLESS-2','-');(artifact_dir/'rust-wayland-quit-after-yes.png').write_bytes(snapshot.stdout)
    out,err=closing.communicate(timeout=10)
    assert closing.returncode==2 and not out,err.decode()
    print('PASS: native close respects confirmation, default No, and confirmed cancellation',flush=True)

    # A 1.5x output has a rounded logical width. Monitor crops must share a
    # boundary rather than both rounding outward into their neighboring output.
    result=call(str(root/'usr/bin/swaymsg'),'output HEADLESS-2 scale 1.5 position -853 0')
    assert result.returncode==0,result.stderr.decode()
    time.sleep(.5)
    layout=call(str(root/'usr/bin/swaymsg'),'-t','get_outputs')
    assert layout.returncode==0,layout.stderr.decode()
    outputs={output['name']:output for output in json.loads(layout.stdout)}
    assert outputs['HEADLESS-2']['scale']==1.5
    assert outputs['HEADLESS-2']['rect']=={'x':-853,'y':0,'width':853,'height':640},outputs['HEADLESS-2']
    full=call(str(binary),'full','--raw');assert full.returncode==0,full.stderr.decode()
    (artifact_dir/'rust-wayland-fractional-full.png').write_bytes(full.stdout)
    composite=Image.open(BytesIO(full.stdout)).convert('RGB')
    assert composite.size==(2816,1152),composite.size
    for index,size in [(0,(1536,1152)),(1,(1280,960))]:
        result=call(str(binary),'screen','--number',str(index),'--raw')
        assert result.returncode==0,result.stderr.decode()
        (artifact_dir/f'rust-wayland-fractional-screen-{index}.png').write_bytes(result.stdout)
        pixels=Image.open(BytesIO(result.stdout)).convert('RGB')
        assert pixels.size==size,(index,pixels.size)
        bounds=(1280,0,2816,1152) if index==0 else (0,0,1280,960)
        assert pixels.tobytes()==composite.crop(bounds).tobytes(),(index,'Crop differs from corresponding portal pixels')
        # The source may include resampling or previously rendered content; its
        # pixels above are authoritative. The adjoining edges must still belong
        # to the blue primary output and red secondary output respectively.
        red,green,blue=pixels.getpixel((0 if index==0 else pixels.width-1,20))
        assert (blue-red>30 if index==0 else red-blue>30),(index,'wrong monitor at shared boundary')
    print('PASS: fractional-scale negative-origin crops have no overlapping neighbor columns',flush=True)


finally:
    for process in reversed(processes):
        if process.poll() is None:
            process.terminate()
            try:process.wait(timeout=5)
            except subprocess.TimeoutExpired:process.kill();process.wait()
    for log in logs:log.close()
