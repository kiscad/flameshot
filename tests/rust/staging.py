#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Validate staged payloads without installing. Usage: staging.py FLAMESHOT_BINARY"""
import os
from pathlib import Path
import shlex
import stat
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

def interface_contract(path):
    tree=ET.parse(path).getroot()
    interface=tree if tree.tag=='interface' else tree.find("interface[@name='org.flameshot.Flameshot']")
    assert interface is not None
    result={}
    for method in interface:
        assert method.tag=='method' and method.attrib['name'] not in result
        arguments=[tuple(sorted(argument.attrib.items())) for argument in method.findall('arg')]
        annotations=[tuple(sorted(annotation.attrib.items())) for annotation in method.findall('annotation')]
        assert len(annotations)==len(set(annotations))
        result[method.attrib['name']]=(arguments,sorted(annotations))
    return result
reference=interface_contract(Path('crates/flameshot/tests/fixtures/qt-dbus-interface.xml'))

binary=Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix='flameshot-stage-') as directory:
    root=Path(directory)
    for number,prefix in enumerate(['/usr','/opt/Flameshot App']):
        stage=root/str(number)
        subprocess.run(['sh','scripts/stage-rust.sh'],env=dict(os.environ,DESTDIR=str(stage),PREFIX=prefix,FLAMESHOT_BINARY=str(binary)),check=True)
        payload=stage/prefix.lstrip('/')
        installed=payload/'bin/flameshot'
        assert installed.read_bytes()==binary.read_bytes()
        assert stat.S_IMODE(installed.stat().st_mode)==0o755
        service=(payload/'share/dbus-1/services/org.flameshot.Flameshot.service').read_text()
        executable=next(line[5:] for line in service.splitlines() if line.startswith('Exec='))
        assert shlex.split(executable)==[prefix+'/bin/flameshot']
        for relative in ['share/man/man1/flameshot.1','share/bash-completion/completions/flameshot','share/zsh/site-functions/_flameshot','share/fish/vendor_completions.d/flameshot.fish','share/dbus-1/interfaces/org.flameshot.Flameshot.xml']:
            assert (payload/relative).is_file(),relative
        assert interface_contract(payload/'share/dbus-1/interfaces/org.flameshot.Flameshot.xml')==reference
        for source, installed_name in [('third-party/winit/LICENSE','winit-LICENSE'),
                                       ('third-party/README.md','winit-patch.txt')]:
            assert (payload/'share/licenses/flameshot'/installed_name).read_bytes()==Path(source).read_bytes()
        for source, relative in [
            ('data/img/app/org.flameshot.Flameshot.svg','share/icons/hicolor/scalable/apps/org.flameshot.Flameshot.svg'),
            ('data/img/app/org.flameshot.Flameshot.png','share/icons/hicolor/128x128/apps/org.flameshot.Flameshot.png'),
        ]:
            assert (payload/relative).read_bytes()==Path(source).read_bytes(),relative
        assert not list(payload.rglob('*.qm'))
        for path in (payload/'share').rglob('*'):
            if path.is_file():assert stat.S_IMODE(path.stat().st_mode)==0o644,path
        subprocess.run(['desktop-file-validate',str(payload/'share/applications/org.flameshot.Flameshot.desktop')],check=True)
        subprocess.run(['appstreamcli','validate','--no-net',str(payload/'share/metainfo/org.flameshot.Flameshot.metainfo.xml')],check=True)
    for number,prefix in enumerate(['relative','/../escape','/opt/../../escape','/opt/..','/opt/bad\nExec=other','/opt/bad\rvalue']):
        stage=root/f'invalid-{number}'
        result=subprocess.run(['sh','scripts/stage-rust.sh'],env=dict(os.environ,DESTDIR=str(stage),PREFIX=prefix,FLAMESHOT_BINARY=str(binary)),capture_output=True)
        assert result.returncode!=0,prefix
        assert not stage.exists(),'Invalid prefix wrote a partial package'
    assert not (root/'escape').exists()
print('PASS: staged bytes, modes, resources, quoted activation path and rejected invalid prefixes')
