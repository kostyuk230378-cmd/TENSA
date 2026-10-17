"""Глубокий аудит: содержимое APK (dex) и установленного Setup (exe/статика).

Печатает только метки и файлы; значения не выводятся.
"""
import io
import os
import re
import shutil
import subprocess
import tempfile
import zipfile

HOME = os.path.expanduser('~')
TENSA = os.path.join(HOME, '.tensa')

def read(path):
    try:
        return io.open(path, encoding='utf-8', errors='replace').read()
    except OSError:
        return ''

needles = []
secrets = read(os.path.join(TENSA, 'secrets.toml'))
for match in re.finditer(r'credential\s*=\s*"([^"]+)"', secrets):
    needles.append(('credential', match.group(1)))
owner = re.search(r'owner_chat_id\s*=\s*"([^"]+)"', secrets)
if owner:
    needles.append(('owner_chat_id', owner.group(1)))
pwa = read(os.path.join(TENSA, 'pwa_token')).strip()
if pwa:
    needles.append(('pwa_token', pwa))

patterns = [
    ('bot-token', re.compile(rb'\b\d{8,10}:[A-Za-z0-9_-]{35}\b')),
    ('openrouter-key', re.compile(rb'sk-or-[A-Za-z0-9]{10,}')),
    ('lan-ip', re.compile(rb'192\.168\.\d{1,3}\.\d{1,3}')),
    ('home-path', re.compile('C:\\\\Users\\\\[A-Za-zА-Яа-яЁё]+'.encode('utf-8'))),
    ('email-address', re.compile(rb'[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}')),
]

def scan(data):
    found = []
    for label, value in needles:
        count = data.count(value.encode('utf-8'))
        if count:
            found.append((label, count))
    for label, pattern in patterns:
        count = len(pattern.findall(data))
        if count:
            found.append((label + ' [шаблон]', count))
    return found

def report(title, name, data):
    hits = scan(data)
    print(f'  {title}: {name}')
    for label, count in hits:
        print(f'    {label}: {count}')

print('== APK (dex и assets) ==')
with zipfile.ZipFile('apps/android-lite/Tensa.apk') as archive:
    for info in archive.infolist():
        if info.file_size > 64 * 1024 * 1024:
            continue  # модель не сканируем построчно
        data = archive.read(info)
        hits = scan(data)
        for label, count in hits:
            print(f'  {info.filename}: {label} ({count})')

print('== Установленный Setup (silent install во временный каталог) ==')
target = os.path.join(tempfile.gettempdir(), 'tensa-audit-install')
shutil.rmtree(target, ignore_errors=True)
setup = os.path.abspath('apps/desktop-lite/TensaSetup.exe')
proc = subprocess.run([setup, '/S', f'/D={target}'], capture_output=True)
print(f'  install exit: {proc.returncode}')
for root, _, names in os.walk(target):
    for name in names:
        path = os.path.join(root, name)
        size = os.path.getsize(path)
        if size > 64 * 1024 * 1024:
            continue
        data = io.open(path, 'rb').read()
        hits = scan(data)
        for label, count in hits:
            rel = os.path.relpath(path, target)
            print(f'  {rel}: {label} ({count})')
uninstall = os.path.join(target, 'Uninstall.exe')
if os.path.exists(uninstall):
    subprocess.run([uninstall, '/S'], capture_output=True)
print('  uninstall отправлен')
