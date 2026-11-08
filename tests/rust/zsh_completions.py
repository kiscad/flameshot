# SPDX-License-Identifier: GPL-3.0-or-later
"""Drive real ZLE completion in a disposable PTY; never execute the completed lines."""
import fcntl
import os
from pathlib import Path
import pty
import re
import select
import shlex
import shutil
import signal
import struct
import tempfile
import termios
import time


def check_zsh(binary, tools, scripts, env, help_text):
    with tempfile.TemporaryDirectory(prefix='flameshot-zsh-') as directory:
        root = Path(directory)
        functions = root/'functions'
        functions.mkdir()
        shutil.copyfile(scripts/'flameshot.zsh', functions/'_flameshot')
        (root/'image file.png').touch()
        output, ready = root/'buffer', root/'ready'
        standard = tools/'usr/share/zsh/functions'
        paths = [functions, standard, *sorted(p for p in standard.rglob('*') if p.is_dir())]
        modules = sorted({p.parent.parent for pattern in
                          ['usr/lib/*/zsh/*/zsh/complete.so', 'usr/lib/zsh/*/zsh/complete.so']
                          for p in tools.glob(pattern)})
        setup = root/'setup.zsh'
        setup.write_text(
            'unset HISTFILE\nPROMPT="test> "\nRPROMPT=""\n'
            'module_path=('+' '.join(shlex.quote(str(p)) for p in modules)+' $module_path)\n'
            'fpath=('+' '.join(shlex.quote(str(p)) for p in paths)+')\n'
            # Only the explicitly supplied distribution functions and local fixture are used.
            'autoload -Uz compinit\ncompinit -u -D || return 1\n'
            'compdef _flameshot flameshot\nzstyle ":completion:*" menu no\n'
            '_fs_capture() { print -r -- "$BUFFER" > '+shlex.quote(str(output))+
            '; BUFFER=""; CURSOR=0; zle reset-prompt }\n'
            'zle -N fs-capture _fs_capture\nbindkey "^X" fs-capture\n'
            'print ready > '+shlex.quote(str(ready))+'\n')
        child_env = dict(env, PATH=str(binary.parent)+os.pathsep+env['PATH'], ZDOTDIR=directory, TERM='xterm')
        zsh = tools/'bin/zsh'
        pid, terminal = pty.fork()
        if pid == 0:
            os.chdir(root)
            os.execve(zsh, [str(zsh), '-f', '-i'], child_env)
        os.set_blocking(terminal, False)
        fcntl.ioctl(terminal, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 120, 0, 0))
        transcript = []

        def wait_for(path):
            deadline = time.monotonic()+10
            while not path.exists():
                assert time.monotonic() < deadline, ''.join(transcript)[-3000:]
                if select.select([terminal], [], [], .05)[0]:
                    try:
                        transcript.append(os.read(terminal, 65536).decode(errors='replace'))
                    except BlockingIOError:
                        pass

        def complete(line):
            output.unlink(missing_ok=True)
            os.write(terminal, (line+'\t\x18').encode())
            wait_for(output)
            return shlex.split(output.read_text())

        try:
            os.write(terminal, ('source '+shlex.quote(str(setup))+'\n').encode())
            wait_for(ready)
            for line, expected in [
                ('flameshot gui --raw --pa', ['flameshot', 'gui', '--raw', '--path']),
                ('flameshot config -n f', ['flameshot', 'config', '-n', 'false']),
                ('flameshot gui -s --pa', ['flameshot', 'gui', '-s', '--path']),
                ('flameshot screen -n 1 --ra', ['flameshot', 'screen', '-n', '1', '--raw']),
                ('flameshot gui --path ima', ['flameshot', 'gui', '--path', 'image file.png']),
                ('flameshot --vers', ['flameshot', '--version']),
            ]:
                assert complete(line) == expected, (line, expected, output.read_text())
            for command, prefix in [('history', 'his'), ('updates', 'upd')]:
                available = bool(re.search(r'^  '+command+r'\s', help_text, re.M))
                assert complete('flameshot '+prefix) == ['flameshot', command if available else prefix]
        finally:
            try:
                os.killpg(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.waitpid(pid, 0)
            os.close(terminal)
    print('PASS: Zsh live completion contexts, optional commands and escaped filenames')
