"""Сборщик установщика: remap путей + нативный сервер. Временный скрипт."""
import io

path = 'apps/desktop-lite/build_installer.ps1'
s = io.open(path, encoding='utf-8', newline='').read()
eol = '\r\n' if '\r\n' in s else '\n'

pairs = [
    ('''Write-Output '== release-сборки (server, cli, bot) =='
Push-Location $repo
try {
    & $cargo build --release -p tensa-server | Out-Null''',
     '''Write-Output '== release-сборки (server, cli, bot) =='
# Пути сборки не должны нести имя пользователя в бинарниках (private data).
$env:RUSTFLAGS = "--remap-path-prefix=$env:USERPROFILE=~"
Push-Location $repo
try {
    & $cargo build --release -p tensa-server --features native | Out-Null'''),
]

for old, new in pairs:
    old = old.replace('\n', eol)
    new = new.replace('\n', eol)
    if new in s:
        print('already patched')
        continue
    if old not in s:
        raise SystemExit('anchor not found')
    s = s.replace(old, new, 1)

io.open(path, 'w', encoding='utf-8', newline='').write(s)
print('patched build_installer.ps1')
