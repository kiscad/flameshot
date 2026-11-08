#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check Chinese desktop-window rendering and lifecycle on private Xvfb.
Usage: localization_smoke.py TOOL_ROOT FLAMESHOT_BINARY
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root,binary=(Path(p).resolve() for p in sys.argv[1:])
artifacts=Path(os.environ.get('FLAMESHOT_TEST_ARTIFACT_DIR',str(binary.parent.parent)))
artifacts.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='flameshot-localization-') as directory:
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
import subprocess,sys,time,os,re
from pathlib import Path
binary,xdotool,artifacts=sys.argv[1:]
config=Path(os.environ['XDG_CONFIG_HOME'])/'flameshot/flameshot.ini'
config.parent.mkdir(exist_ok=True)
def tool(*args):
    return subprocess.run([xdotool,*args],env=dict(os.environ,LC_ALL="C.UTF-8"),capture_output=True,timeout=5)
def window(title):
    deadline=time.monotonic()+10
    while time.monotonic()<deadline:
        found=tool('search','--onlyvisible','--name','^Flameshot')
        for identifier in reversed(found.stdout.decode().splitlines()):
            name=tool('getwindowname',identifier).stdout.decode().rstrip('\n')
            if re.search(title,name):return identifier
        time.sleep(.1)
    names=tool('search','--onlyvisible','--name','Flameshot').stdout.decode().splitlines()
    raise AssertionError('Missing '+title+'; windows: '+repr([tool('getwindowname',name).stdout for name in names]))
for locale,title in [('zh_CN','Flameshot 启动器'),('zh_TW','Flameshot 啟動器')]:
    config.write_text(f'[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\nuiLanguage={locale}\nsavePath={config.parent}\n')
    process=subprocess.Popen([binary,'launcher'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        launcher=window('^'+title+'$')
        tool('windowfocus','--sync',launcher)
        time.sleep(.5)
        captured=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
        assert captured.returncode==0,captured.stderr.decode()
        Path(artifacts,f'rust-launcher-{locale}.png').write_bytes(captured.stdout)
        tool('key','Return')
        capture=window('^Flameshot$')
        tool('windowfocus','--sync',capture)
        time.sleep(.3)
        for stage in ['editor','save-dialog']:
            if stage=='save-dialog':
                tool('key','ctrl+a')
                tool('key','ctrl+s')
                time.sleep(.5)
            captured=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
            assert captured.returncode==0,captured.stderr.decode()
            Path(artifacts,f'rust-{stage}-{locale}.png').write_bytes(captured.stdout)
        tool('key','Escape') # Cancel Save, leaving the capture editor alive.
        time.sleep(.3)
        assert process.poll() is None
        assert tool('getwindowname',capture).returncode==0
        tool('key','ctrl+o')
        time.sleep(.7)
        captured=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
        assert captured.returncode==0,captured.stderr.decode()
        Path(artifacts,f'rust-open-with-{locale}.png').write_bytes(captured.stdout)
        tool('key','Escape') # Close Open With without launching an application.
        time.sleep(.3)
        tool('key','Escape')
        launcher=window('^'+title+'$')
        tool('windowfocus','--sync',launcher)
        time.sleep(.3)
        tool('key','Escape')
        out,err=process.communicate(timeout=10)
        assert process.returncode==0,err.decode()
    finally:
        if process.poll() is None:process.kill()
        out,err=process.communicate()
        if process.returncode:print(err.decode(),file=sys.stderr)
    print('PASS:',locale,'launcher/editor/save-dialog rendering and capture/cancel lifecycle')
    config_title={'zh_CN':'Flameshot 配置','zh_TW':'Flameshot 設定'}[locale]
    original_config=config.read_bytes()
    process=subprocess.Popen([binary,'config'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        identifier=window('^'+config_title+'$')
        tool('windowfocus','--sync',identifier)
        time.sleep(.5)
        for section in ['top','bottom']:
            if section=='bottom':
                tool('mousemove','--window',identifier,'150','300')
                tool('click','--repeat','60','--delay','10','5')
                time.sleep(.5)
            captured=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
            assert captured.returncode==0,captured.stderr.decode()
            Path(artifacts,f'rust-config-{locale}-{section}.png').write_bytes(captured.stdout)
            if section=='top':top_capture=captured.stdout
            else:assert captured.stdout!=top_capture,'Configuration did not scroll'
        tool('mousemove','--window',identifier,'35','393')
        tool('click','1')
        time.sleep(.5)
        captured=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
        assert captured.returncode==0,captured.stderr.decode()
        Path(artifacts,f'rust-config-{locale}-saved.png').write_bytes(captured.stdout)
        tool('key','Escape')
        out,err=process.communicate(timeout=10)
        assert process.returncode==0,err.decode()
        assert config.read_bytes()==original_config
    finally:
        if process.poll() is None:process.kill();process.wait()
    print('PASS:',locale,'localized configuration title, scrolling, Escape and unchanged INI')
    if b'history' in subprocess.run([binary,'--help'],capture_output=True,timeout=5).stdout:
        from PIL import Image
        history=Path(os.environ['XDG_CACHE_HOME'])/'flameshot/history'
        history.mkdir(parents=True,exist_ok=True)
        record=history/'imgur-localtoken-localimage.png'
        Image.new('RGB',(80,60),(32,64,96)).save(record)
        original=record.read_bytes()
        history_title={'zh_CN':'Flameshot 上传历史记录','zh_TW':'Flameshot 上傳歷史記錄'}[locale]
        process=subprocess.Popen([binary,'history'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        try:
            identifier=window('^'+history_title+'$')
            tool('windowfocus','--sync',identifier)
            time.sleep(.5)
            captured=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
            assert captured.returncode==0,captured.stderr.decode()
            Path(artifacts,f'rust-history-{locale}.png').write_bytes(captured.stdout)
            tool('key','Escape')
            out,err=process.communicate(timeout=10)
            assert process.returncode==0,err.decode()
            assert record.read_bytes()==original
        finally:
            if process.poll() is None:process.kill();process.wait()
        print('PASS:',locale,'localized history title and intact local record')


    import struct,io
    from PIL import Image
    image=Image.new('RGBA',(400,300),(255,0,0,255));image.paste((0,255,0,255),(200,0,400,300))
    data=io.BytesIO();image.save(data,format='PNG')
    pin_file=config.parent/'localized-pin.bin'
    pin_file.write_bytes(struct.pack('>I',1)+data.getvalue()+struct.pack('>iiiid',100,100,499,399,1.0))
    with pin_file.open('rb') as source:
        process=subprocess.Popen([binary,'internal-pin'],stdin=source,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        identifier=window('^flameshot-pin$')
        tool('windowfocus','--sync',identifier);time.sleep(.5)
        def pin_size():
            values=dict(line.split('=',1) for line in tool('getwindowgeometry','--shell',identifier).stdout.decode().splitlines() if '=' in line)
            return int(values['WIDTH']),int(values['HEIGHT'])
        def wait_size(expected):
            deadline=time.monotonic()+5
            while pin_size()!=expected:
                assert time.monotonic()<deadline,(locale,pin_size(),expected)
                time.sleep(.1)
        wait_size((400,300))
        tool('mousemove','--window',identifier,'20','20');tool('click','3');time.sleep(.3)
        captured=subprocess.run([binary,'full','--raw'],capture_output=True,timeout=10)
        assert captured.returncode==0,captured.stderr.decode()
        Path(artifacts,f'rust-pin-{locale}-menu.png').write_bytes(captured.stdout)
        tool('key','--delay','150','Down','Down','Down','Return')
        wait_size((300,400))
        tool('mousemove','--window',identifier,'150','200');tool('click','4')
        wait_size((330,440))
        # A separate complete click must apply exactly one further step, and
        # reverse scrolling must not be suppressed by a timing debounce.
        tool('click','4');wait_size((363,484))
        tool('click','5');wait_size((330,440))
        tool('mousemove','--window',identifier,'20','20');tool('click','3');time.sleep(.3)
        tool('key','--delay','150','Down','Down','Down','Down','Down','Return')
        wait_size((300,400))
        tool('key','Escape');out,err=process.communicate(timeout=10)
        assert process.returncode==0,err.decode()
    finally:
        if process.poll() is None:process.kill();process.wait()
    print('PASS:',locale,'localized pin menu, rotation, zoom reset and close')

'''
        subprocess.run(['dbus-run-session','--config-file',str(config),'--',sys.executable,'-c',driver,str(binary),str(root/'usr/bin/xdotool'),str(artifacts)],env=env,check=True,timeout=100)
    finally:
        display.terminate();display.wait(timeout=5)
