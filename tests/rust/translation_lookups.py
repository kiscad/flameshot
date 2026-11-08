#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check ordinary quoted app translation calls and local tr wrappers against TS keys.
This does not infer dynamic labels/raw strings or require unfinished translations to be complete.
"""
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET

project = Path(__file__).resolve().parents[2]
keys = set()
catalogs = list((project/'data/translations').glob('*.ts'))
assert catalogs, 'No translation catalogs'
for path in catalogs:
    for context in ET.parse(path).getroot().findall('context'):
        for message in context.findall('message'):
            translation = message.find('translation')
            if translation is not None and translation.get('type') not in ('vanished','obsolete'):
                keys.add((context.findtext('name'), message.findtext('source')))

literal = r'"(?:[^"\\]|\\.)*"'
pair = re.compile(r'\btr\(\s*('+literal+r')\s*,\s*('+literal+r')', re.S)
wrapper = re.compile(r'(?:fn tr\([^)]*\)[^{]*\{|let tr\s*=\s*\|\w+\|)\s*crate::i18n::tr\(\s*('+literal+r')\s*,\s*\w+\s*\)', re.S)
single = re.compile(r'\btr\(\s*('+literal+r')\s*\)', re.S)
missing = []
checked = 0
for path in (project/'crates/flameshot/src').rglob('*.rs'):
    source = path.read_text(encoding='utf-8')
    calls = [(match.start(), tuple(map(json.loads, match.groups()))) for match in pair.finditer(source)]
    contexts = {json.loads(match.group(1)) for match in wrapper.finditer(source)}
    if contexts:
        assert len(contexts) == 1, f'Ambiguous local tr wrapper: {path}'
        context = contexts.pop()
        calls += [(match.start(), (context, json.loads(match.group(1)))) for match in single.finditer(source)]
    for position, key in calls:
        checked += 1
        if key not in keys:
            missing.append(f'{path.relative_to(project)}:{source.count(chr(10),0,position)+1}: {key!r}')
assert checked, 'No literal translation calls found'
assert not missing, 'Missing active catalog entries:\n'+'\n'.join(missing)
print(f'PASS: {checked} literal translation calls match source keys in {len(catalogs)} catalogs')
