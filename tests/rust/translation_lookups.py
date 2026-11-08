#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check quoted translation calls, local tr wrappers and diagnostic templates against TS keys.
This does not infer dynamic labels/raw strings or require unfinished translations to be complete.
"""
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET

project = Path(__file__).resolve().parents[2]
keys = set()
finished = []
catalogs = list((project/'data/translations').glob('*.ts'))
assert catalogs, 'No translation catalogs'
for path in catalogs:
    for context in ET.parse(path).getroot().findall('context'):
        for message in context.findall('message'):
            translation = message.find('translation')
            if translation is not None and translation.get('type') not in ('vanished','obsolete'):
                keys.add((context.findtext('name'), message.findtext('source')))
                value = ''.join(translation.itertext())
                if translation.get('type') != 'unfinished' and value:
                    finished.append((path, context.findtext('name'), message.findtext('source'), value))

literal = r'"(?:[^"\\]|\\.)*"'
pair = re.compile(r'\b(?:tr|localized|dbus_menu_label)\(\s*('+literal+r')\s*,\s*('+literal+r')', re.S)
wrapper = re.compile(r'(?:fn tr\([^)]*\)[^{]*\{|let tr\s*=\s*\|\w+\|)\s*crate::i18n::tr\(\s*('+literal+r')\s*,\s*\w+\s*\)', re.S)
single = re.compile(r'\b(?:tr|localized|dbus_menu_label)\(\s*('+literal+r')\s*,?\s*\)', re.S)
missing = []
used = set()
checked = 0
for path in (project/'crates/flameshot/src').rglob('*.rs'):
    source = path.read_text(encoding='utf-8')
    calls = [(match.start(), tuple(map(json.loads, match.groups()))) for match in pair.finditer(source)]
    contexts = {json.loads(match.group(1)) for match in wrapper.finditer(source)}
    if contexts:
        assert len(contexts) == 1, f'Ambiguous local tr wrapper: {path}'
        context = contexts.pop()
        calls += [(match.start(), (context, json.loads(match.group(1)))) for match in single.finditer(source)]
    if path.name == 'buttons.rs':
        context = re.search(r'pub const CATALOG_CONTEXT: &str = ('+literal+r');', source)
        catalog = re.search(r'pub const CATALOG:.*?= &\[(.*?)\];', source, re.S)
        assert context and catalog, 'Missing button translation catalog declaration'
        labels = list(re.finditer(r'\(\s*[0-9]+\s*,\s*('+literal+r')\s*\)', catalog.group(1)))
        assert len(labels) == len(re.findall(r'\(\s*[0-9]+\s*,', catalog.group(1))) > 0
        calls += [(catalog.start(1)+label.start(), (json.loads(context.group(1)), json.loads(label.group(1)))) for label in labels]
    for position, key in calls:
        used.add(key)
        checked += 1
        if key not in keys:
            missing.append(f'{path.relative_to(project)}:{source.count(chr(10),0,position)+1}: {key!r}')
assert checked, 'No literal translation calls found'
assert not missing, 'Missing active catalog entries:\n'+'\n'.join(missing)
markers = re.compile(r'%L?[0-9]+|%n')
for path, context, source, value in finished:
    if (context, source) in used:
        assert set(markers.findall(source)) == set(markers.findall(value)), f'Placeholder mismatch: {path.name}: {context}/{source}'
print(f'PASS: {checked} translation calls/catalog labels match {len(catalogs)} catalogs; finished used templates preserve placeholders')
