# Сборка Windows-установщика Tensa (TensaSetup.exe) — [derived] тестовый
# дистрибутив: ядро + PWA + CLI + Telegram-бот. Полноценное десктоп-приложение
# из ТЗ apps/desktop — отдельный этап.
#
# Требования (ставятся без админ-прав, вне кириллицы в пути):
#   C:\tensa-tools\nsis-3.11         (makensis.exe)
#   Rust release-сборки в target\release (собираются скриптом при отсутствии)
#
# Запуск (из корня репозитория):
#   powershell -ExecutionPolicy Bypass -File apps/desktop-lite/build_installer.ps1
#
# Результат: apps/desktop-lite/TensaSetup.exe

$ErrorActionPreference = 'Stop'

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Split-Path -Parent (Split-Path -Parent $here)
$stage = 'C:\tensa-build\stage'
$makensis = 'C:\tensa-tools\nsis-3.11\makensis.exe'
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'

if (-not (Test-Path $makensis)) { throw "Не найден makensis: $makensis" }
if (-not (Test-Path $cargo)) { throw "Не найден cargo: $cargo" }

Write-Output '== release-сборки (server, cli, bot) =='
# Пути сборки не должны нести имя пользователя в бинарниках (private data).
$env:RUSTFLAGS = "--remap-path-prefix=$env:USERPROFILE=~"
Push-Location $repo
try {
    & $cargo build --release -p tensa-server --features native | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'release tensa-server failed' }
    & $cargo build --release -p tensa-cli | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'release tensa-cli failed' }
    & $cargo build --release -p tensa-tools --example telegram_flow | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'release tensa-bot (example) failed' }

    Write-Output '== staging =='
    New-Item -ItemType Directory -Force $stage | Out-Null
    Copy-Item (Join-Path $repo 'target\release\tensa-server.exe') (Join-Path $stage 'tensa-server.exe') -Force
    Copy-Item (Join-Path $repo 'target\release\tensa.exe') (Join-Path $stage 'tensa-cli.exe') -Force
    Copy-Item (Join-Path $repo 'target\release\examples\telegram_flow.exe') (Join-Path $stage 'tensa-bot.exe') -Force
    Copy-Item (Join-Path $repo 'target\release\tensa-desktop.exe') (Join-Path $stage 'tensa-desktop.exe') -Force
    Copy-Item (Join-Path $repo 'target\release\tensa-whisper.exe') (Join-Path $stage 'tensa-whisper.exe') -Force

    $pwaOut = Join-Path $stage 'pwa'
    Remove-Item $pwaOut -Recurse -Force -ErrorAction SilentlyContinue
    # Сервер ищет статику в <cwd>\pwa\static — раскладка установки должна совпадать.
    New-Item -ItemType Directory -Force (Join-Path $pwaOut 'static') | Out-Null
    Copy-Item (Join-Path $repo 'pwa\static\*') (Join-Path $pwaOut 'static') -Recurse -Force

    # Vite при пересборке вычищает outDir — APK в поставку Windows не входит
    # (он крупный; телефон ставит его из GitHub Releases или с сервера ядра).
    Remove-Item (Join-Path $pwaOut 'static\Tensa.apk') -Force -ErrorAction SilentlyContinue

    # Локальная модель эмбеддингов e5-small — прямо в поставку (Требование Архитектора).
    $modelsSrc = if ($env:TENSA_MODELS_SRC) { $env:TENSA_MODELS_SRC } else { 'C:\tensa-build\models' }
    $modelsOut = Join-Path $stage 'models\multilingual-e5-small'
    if (Test-Path (Join-Path $modelsSrc 'multilingual-e5-small')) {
        Remove-Item (Join-Path $stage 'models') -Recurse -Force -ErrorAction SilentlyContinue
        New-Item -ItemType Directory -Force $modelsOut | Out-Null
        Copy-Item (Join-Path $modelsSrc 'multilingual-e5-small\*') $modelsOut -Recurse -Force
        Write-Output ("модель в поставке: {0:N1} МБ" -f ((Get-ChildItem $modelsOut -Recurse -File | Measure-Object Length -Sum).Sum / 1MB))
        $whisperOut = Join-Path $stage 'models\whisper'
        New-Item -ItemType Directory -Force $whisperOut | Out-Null
        $whisperName = 'ggml-small-q5_1.bin'
        $whisperSrc = Join-Path $modelsSrc $whisperName
        if (-not (Test-Path $whisperSrc)) { $whisperSrc = Join-Path $env:USERPROFILE ".tensa\models\$whisperName" }
        if (-not (Test-Path $whisperSrc)) { throw "Нет $whisperName (whisper) в $modelsSrc" }
        Copy-Item $whisperSrc (Join-Path $whisperOut $whisperName) -Force
        Write-Output ("whisper в поставке: {0:N1} МБ" -f ((Get-Item (Join-Path $whisperOut $whisperName)).Length / 1MB))
    } else {
        throw "Нет файлов модели в $modelsSrc — модель обязана быть в установщике (TENSA_MODELS_SRC)"
    }

    Copy-Item (Join-Path $here 'Tensa.cmd') $stage -Force
    Copy-Item (Join-Path $here 'TensaBot.cmd') $stage -Force
    Copy-Item (Join-Path $here 'download_models.ps1') $stage -Force
    Copy-Item (Join-Path $here 'DownloadModels.cmd') $stage -Force
    Copy-Item (Join-Path $here 'README.txt') $stage -Force

    Write-Output '== иконка (ico из PWA-иконки) =='
    $icon = Join-Path $stage 'tensa.ico'
    python -c "from PIL import Image; Image.open(r'pwa/static/icons/icon-512.png').save(r'$icon', sizes=[(16,16),(32,32),(48,48),(64,64),(128,128),(256,256)])"
    if ($LASTEXITCODE -ne 0) { throw 'не удалось сгенерировать tensa.ico (Pillow)' }

    Write-Output '== makensis =='
    Push-Location $here
    try {
        & $makensis "/DSTAGE=$stage" 'tensa.nsi'
        if ($LASTEXITCODE -ne 0) { throw 'makensis failed' }
    } finally {
        Pop-Location
    }

    $setup = Join-Path $here 'TensaSetup.exe'
    if (-not (Test-Path $setup)) { throw 'TensaSetup.exe не создан' }
    Get-Item $setup | ForEach-Object {
        Write-Output ("готово: {0} ({1:N1} МБ)" -f $_.FullName, ($_.Length / 1MB))
    }
} finally {
    Pop-Location
}
