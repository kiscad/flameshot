#!/usr/bin/env python3
"""Exercise the Snap launcher using private sockets and paths only."""
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile

launcher = Path(__file__).resolve().parents[2] / 'data/snap/launch'
with tempfile.TemporaryDirectory(prefix='flameshot snap ') as temporary:
    root = Path(temporary)
    payload = root / 'payload'
    libraries = payload / 'usr/lib/x86_64-linux-gnu'
    libraries.mkdir(parents=True)
    runtime = root / 'snap.flameshot'
    runtime.mkdir()
    env = dict(os.environ, SNAP=str(payload), XDG_RUNTIME_DIR=str(runtime),
               WAYLAND_DISPLAY='wayland-test', LD_LIBRARY_PATH='/gpu-libraries')
    source = root / 'wayland-test'
    target = runtime / 'wayland-test'
    with socket.socket(socket.AF_UNIX) as compositor:
        compositor.bind(str(source))
        def launch():
            result = subprocess.run(
                [str(launcher), sys.executable, '-c',
                 'import json,os,sys; print(json.dumps([sys.argv[1:], dict(os.environ)]))',
                 'space in argument', '$(literal)'],
                env=env, capture_output=True, text=True, check=True)
            arguments, observed = json.loads(result.stdout)
            assert arguments == ['space in argument', '$(literal)']
            assert observed['XDG_RUNTIME_DIR'] == str(runtime)
            assert observed['LD_LIBRARY_PATH'] == f'/gpu-libraries:{libraries}'
        launch()
        assert target.is_symlink() and target.resolve() == source
        launch()  # Repeated launches preserve the existing socket link.
        target.unlink()
        target.write_text('existing entry')
        launch()
        assert target.read_text() == 'existing entry'
        target.unlink()
        target.symlink_to(root / 'missing')
        launch()
        assert target.readlink() == root / 'missing'
        target.unlink()
        env['WAYLAND_DISPLAY'] = str(source)
        launch()
        assert not target.exists()
print('Snap launcher: arguments, library order, Wayland socket and existing paths passed')
