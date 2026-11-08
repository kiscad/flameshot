#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Install/remove/reinstall an RPM in a disposable Fedora container.
Usage: rpm_install.py PACKAGE.rpm IMAGE SOURCE_REVISION
The image must already be available locally. No host directories are mounted.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import uuid

package = Path(sys.argv[1]).resolve()
image, revision = sys.argv[2:]
repository = Path(__file__).resolve().parents[2]
artifacts = repository / 'target/rpm-install-validation'
artifacts.mkdir(parents=True, exist_ok=True)
name = 'flameshot-test-rpm-' + uuid.uuid4().hex[:12]
label = 'org.flameshot.test=rpm-install'
logs = []


def docker(*args):
    result = subprocess.run(['docker', *args], capture_output=True, text=True, timeout=600)
    logs.append('$ docker ' + ' '.join(args) + '\n' + result.stdout + result.stderr)
    result.check_returncode()
    return result.stdout


def run(*args):
    return docker('exec', name, *args)


image_id = docker('image', 'inspect', '--format', '{{.Id}}', image).strip()
created = False
try:
    docker('run', '--pull=never', '-d', '--name', name, '--label', label,
           image_id, 'sleep', 'infinity')
    created = True
    run('mkdir', '-p', '/opt/review/config/flameshot')
    docker('cp', str(package), name + ':/opt/review/flameshot.rpm')
    config = '[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\n\n[External]\nkeep=unchanged\n'
    with tempfile.TemporaryDirectory(prefix='flameshot-rpm-config-') as directory:
        path = Path(directory) / 'flameshot.ini'
        path.write_text(config)
        docker('cp', str(path), name + ':/opt/review/config/flameshot/flameshot.ini')
    for cycle in range(2):
        run('dnf', '-y', '--setopt=install_weak_deps=False', 'install', '/opt/review/flameshot.rpm')
        assert not run('rpm', '-V', 'flameshot').strip()
        assert run('cat', '/opt/review/config/flameshot/flameshot.ini') == config
        run('env', 'XDG_CONFIG_HOME=/opt/review/config', 'flameshot', 'config', '--check')
        help_text = run('env', 'XDG_CONFIG_HOME=/opt/review/config', 'flameshot', '--help')
        assert 'history' in help_text and 'updates' not in help_text
        linkage = run('ldd', '/usr/bin/flameshot')
        assert 'not found' not in linkage and 'libQt' not in linkage
        for target, source in [
            ('licenses/flameshot/winit-LICENSE', 'third-party/winit/LICENSE'),
            ('licenses/flameshot/winit-patch.txt', 'third-party/README.md'),
            ('dbus-1/interfaces/org.flameshot.Flameshot.xml', 'data/dbus/org.flameshot.Flameshot.xml'),
        ]:
            assert run('cat', '/usr/share/' + target) == (repository / source).read_text()
        binary_hash = run('sha256sum', '/usr/bin/flameshot').split()[0]
        version = run('flameshot', '--version').strip()
        run('dnf', '-y', 'remove', 'flameshot')
        for path in ['/usr/bin/flameshot', '/usr/share/applications/org.flameshot.Flameshot.desktop',
                     '/usr/share/dbus-1/services/org.flameshot.Flameshot.service']:
            run('test', '!', '-e', path)
        assert run('cat', '/opt/review/config/flameshot/flameshot.ini') == config
    report = {
        'source_revision': revision, 'package': str(package),
        'package_sha256': hashlib.sha256(package.read_bytes()).hexdigest(),
        'image': image, 'image_id': image_id, 'binary_sha256': binary_hash,
        'version': version,
        'checks': ['dependency-resolved install', 'remove/reinstall', 'RPM verification',
                   'user INI preservation', 'licenses and D-Bus interface',
                   'Imgur enabled/update command disabled', 'no missing ELF libraries or Qt'],
        'desktop': 'not exercised by this fixture',
    }
    (artifacts / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print('PASS: Fedora RPM installation, removal/reinstallation, resources and configuration preservation')
finally:
    if created:
        docker('rm', '-f', name)
    (artifacts / 'install.log').write_text('\n'.join(logs))
