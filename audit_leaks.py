"""Аудит утечек: секреты из ~/.tensa не должны встречаться в git и бинарниках.

Печатает только метки и пути; значения секретов не выводятся.
"""
import io
import os
import re
import subprocess
import sys

HOME = os.path.expanduser('~')
TENSA = os.path.join(HOME, '.tensa')

def read(path):
    try:
        return io.open(path, encoding='utf-8', errors='replace').read()
    except OSError:
        return ''

needles = []  # (label, value)

secrets = read(os.path.join(TENSA, 'secrets.toml'))
for match in re.finditer(r'credential\s*=\s*"([^"]+)"', secrets):
    needles.append(('credential из secrets.toml', match.group(1)))
owner = re.search(r'owner_chat_id\s*=\s*"([^"]+)"', secrets)
if owner:
    needles.append(('owner_chat_id', owner.group(1)))
pwa = read(os.path.join(TENSA, 'pwa_token')).strip()
if pwa:
    needles.append(('pwa_token', pwa))

# Обобщённые шаблоны (без конкретных значений)
patterns = [
    ('bot-token', re.compile(rb'\b\d{8,10}:[A-Za-z0-9_-]{35}\b')),
    ('openrouter-key', re.compile(rb'sk-or-[A-Za-z0-9]{10,}')),
    ('private-key', re.compile(rb'BEGIN [A-Z ]*PRIVATE KEY')),
    ('lan-ip', re.compile(rb'192\.168\.\d{1,3}\.\d{1,3}')),
    ('home-path', re.compile('C:\\\\Users\\\\[A-Za-zА-Яа-яЁё]+'.encode('utf-8'))),
]

def scan_bytes(label, data, where, hits):
    for needle_label, value in needles:
        raw = value.encode('utf-8')
        count = data.count(raw)
        if count:
            hits.append((needle_label, where, count))
    for pattern_label, pattern in patterns:
        count = len(pattern.findall(data))
        if count:
            hits.append((pattern_label + ' [шаблон]', where, count))

# 1. Трекаемые файлы
files = subprocess.run(['git', 'ls-files', '-z'], capture_output=True).stdout.split(b'\0')
hits = []
for raw in files:
    if not raw:
        continue
    path = raw.decode('utf-8', errors='replace')
    try:
        data = io.open(path, 'rb').read()
    except OSError:
        continue
    scan_bytes('file', data, path, hits)

print('== 1. Трекаемые файлы ==')
if not hits:
    print('  чисто: секретов и шаблонов не найдено')
for label, where, count in hits[:60]:
    print(f'  {label}: {where} (совпадений: {count})')

# 2. История git (весь diff-поток)
log = subprocess.run(['git', 'log', '-p', '--all'], capture_output=True).stdout
history_hits = []
scan_bytes('history', log, 'git log -p --all', history_hits)
print('== 2. История git ==')
if not history_hits:
    print('  чисто: в диффах истории секретов/шаблонов нет')
for label, where, count in history_hits[:40]:
    print(f'  {label}: {where} (совпадений: {count})')

# 3. Бинарники, которые публикуются (Release) или раздаются сервером
targets = [
    'apps/desktop-lite/TensaSetup.exe',
    'apps/android-lite/Tensa.apk',
    'apps/android-lite/assets/multilingual-e5-small/model_O4.onnx',
    'target/release/tensa-server.exe',
    'target/release/tensa.exe',
    'target/release/examples/telegram_flow.exe',
]
for root, _, names in os.walk('pwa/static'):
    for name in names:
        targets.append(os.path.join(root, name))

print('== 3. Бинарники и статика ==')
binary_hits = []
for path in targets:
    if not os.path.isfile(path):
        continue
    data = io.open(path, 'rb').read()
    scan_bytes('bin', data, path, binary_hits)
if not binary_hits:
    print('  чисто: в установщике/APK/бинарях/статике секретов нет')
seen = set()
for label, where, count in binary_hits:
    key = (label, where)
    if key in seen:
        continue
    seen.add(key)
    print(f'  {label}: {where} (совпадений: {count})')

# 4. Проверка, что личный корень не в репозитории
print('== 4. Личный корень ==')
print(f'  ~/.tensa в git: {"да" if b".tensa" in files else "нет"}')

sys.exit(0)
