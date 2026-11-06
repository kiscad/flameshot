#!/usr/bin/env python3
"""Check shell completions against the executable without starting a desktop.
Usage: completions.py FLAMESHOT_BINARY [EXTRACTED_SHELL_TOOL_ROOT]
"""
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

project = Path(__file__).resolve().parents[2]
binary = Path(sys.argv[1]).resolve()
scripts = project / 'data/shell-completion'
env = dict(os.environ, PATH=str(binary.parent)+os.pathsep+os.environ['PATH'])
help_text = subprocess.check_output([binary, '--help'], text=True, env=env)
with tempfile.TemporaryDirectory(prefix='flameshot completion ') as directory:
    Path(directory, 'image file.png').touch()
    def bash(words):
        code = 'source "$1"; shift; COMP_WORDS=("$@"); COMP_CWORD=$((${#COMP_WORDS[@]}-1)); _flameshot; printf "%s\\0" "${COMPREPLY[@]}"'
        result = subprocess.run(['bash', '--noprofile', '--norc', '-c', code, 'test', str(scripts/'flameshot.bash'), *words],
                                env=env, cwd=directory, text=True, capture_output=True, check=True)
        return set(filter(None, result.stdout.split('\0')))
    root = bash(['flameshot', ''])
    assert {'gui','screen','full','config','launcher','--help','--version','-v','-V'} <= root
    for optional in ['history','updates']:
        assert (optional in root) == bool(re.search(r'^  '+optional+r'\s', help_text, re.M))
    for command in ['gui','screen','full','config','launcher']:
        usage = subprocess.check_output([binary,command,'--help'],text=True,env=env)
        expected = set(re.findall(r'--[a-z][a-z-]*', usage))
        assert bash(['flameshot',command,'--']) == expected, command
    assert '--path' in bash(['flameshot','gui','--raw','--p'])
    assert '--clipboard' in bash(['flameshot','full','--delay','100','--c'])
    assert bash(['flameshot','config','-n','f']) == {'false'}
    assert bash(['flameshot','screen','-n','']) == set()
    assert not {'true','false'} & bash(['flameshot','gui','-s',''])
    assert bash(['flameshot','config','--filename','']) == set()
    assert bash(['flameshot','gui','--','--p']) == set()
    assert bash(['flameshot','gui','--path','image']) == {'image file.png'}
    print('PASS: Bash command context, values, filenames and CLI option parity')

    if len(sys.argv) > 2:
        tools = Path(sys.argv[2]).resolve()
        shell_env = dict(env, LD_LIBRARY_PATH=str(tools/'usr/lib/x86_64-linux-gnu')+':'+env.get('LD_LIBRARY_PATH',''))
        fish = tools/'usr/bin/fish'
        zsh = tools/'bin/zsh'
        subprocess.run([fish,'--no-config','-n',scripts/'flameshot.fish'],env=shell_env,check=True)
        subprocess.run([zsh,'-n',scripts/'flameshot.zsh'],env=shell_env,check=True)
        def fish_complete(line):
            code = 'set -p fish_function_path $argv[1]; source $argv[2]; complete -C $argv[3]'
            result = subprocess.run([fish,'--no-config','-c',code,'--',tools/'usr/share/fish/functions',scripts/'flameshot.fish',line],
                                    env=shell_env,cwd=directory,capture_output=True,text=True,check=True)
            return {line.split('\t',1)[0] for line in result.stdout.splitlines()}
        assert fish_complete('flameshot config -n ') == {'true','false'}
        assert '--notifications' in fish_complete('flameshot config --not')
        assert '--path' in fish_complete('flameshot gui --raw --p')
        assert not {'true','false'} & fish_complete('flameshot gui -s ')
        assert {'-v','-V'} <= fish_complete('flameshot -')
        for optional in ['history','updates']:
            assert (optional in fish_complete('flameshot ')) == bool(re.search(r'^  '+optional+r'\s',help_text,re.M))
        print('PASS: Fish live completion and Fish/Zsh syntax')
