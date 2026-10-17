# Сборка Tensa Android (lite) — нативного клиента секретаря (Tensa.apk).
#
# [derived] Это нативный Java-клиент платформенными View (без Flutter/Gradle):
# чат с большой моделью через OpenRouter, одобрения/заметки/статус ядра Tensa
# по HTTP. Полное приложение из ТЗ apps/android (Kotlin + uniffi + локальная
# модель на телефоне) — следующий этап.
#
# Требования (ставятся без админ-прав, вне кириллицы в пути):
#   C:\tensa-tools\jdk            (Microsoft OpenJDK 17)
#   C:\tensa-tools\android-sdk    (platforms;android-34, build-tools;34.0.0)
#
# Запуск (из корня репозитория):
#   powershell -ExecutionPolicy Bypass -File apps/android-lite/build_apk.ps1
#
# Результат: apps/android-lite/Tensa.apk (подписан debug-ключом) и копия
# в pwa/static/Tensa.apk — скачивание с сервера ядра:
#   http://<ip-пк>:8765/static/Tensa.apk

$ErrorActionPreference = 'Stop'

if (-not $env:JAVA_HOME) {
    $env:JAVA_HOME = (Get-ChildItem 'C:\tensa-tools\jdk' -Directory |
        Select-Object -First 1).FullName
}
if (-not $env:ANDROID_HOME) {
    $env:ANDROID_HOME = 'C:\tensa-tools\android-sdk'
}

$bt = Join-Path $env:ANDROID_HOME 'build-tools\34.0.0'
$platform = Join-Path $env:ANDROID_HOME 'platforms\android-34\android.jar'
foreach ($tool in @(
    (Join-Path $env:JAVA_HOME 'bin\javac.exe'),
    (Join-Path $env:JAVA_HOME 'bin\jar.exe'),
    (Join-Path $env:JAVA_HOME 'bin\keytool.exe'),
    (Join-Path $bt 'aapt2.exe'),
    (Join-Path $bt 'd8.bat'),
    (Join-Path $bt 'zipalign.exe'),
    (Join-Path $bt 'apksigner.bat'),
    $platform
)) {
    if (-not (Test-Path $tool)) {
        throw "Не найден инструмент: $tool (проверь JAVA_HOME/ANDROID_HOME и пакеты SDK)"
    }
}

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$build = Join-Path $here 'build'
$out = Join-Path $here 'Tensa.apk'
New-Item -ItemType Directory -Force (Join-Path $build 'classes') | Out-Null
Remove-Item (Join-Path $build 'classes\*') -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $build 'd8') -Recurse -Force -ErrorAction SilentlyContinue

Write-Output '== aapt2 link (манифест + assets) =='
$assetsDir = Join-Path $here 'assets'
$assetsArgs = @()
if (Test-Path (Join-Path $assetsDir 'multilingual-e5-small\model_O4.onnx')) {
    $assetsArgs = @('-A', $assetsDir)
    Write-Output 'assets: модель e5-small включена в APK'
} else {
    Write-Warning "Нет assets\multilingual-e5-small — APK будет без модели"
}
& (Join-Path $bt 'aapt2.exe') link -o (Join-Path $build 'base.apk') `
    -I $platform --manifest (Join-Path $here 'AndroidManifest.xml') `
    --min-sdk-version 26 --target-sdk-version 34 @assetsArgs
if ($LASTEXITCODE -ne 0) { throw 'aapt2 link failed' }

Write-Output '== javac (все классы приложения) =='
$sources = Get-ChildItem (Join-Path $here 'java') -Recurse -Filter *.java |
    ForEach-Object { $_.FullName }
& (Join-Path $env:JAVA_HOME 'bin\javac.exe') -source 8 -target 8 -encoding UTF-8 `
    -bootclasspath $platform -classpath $platform -d (Join-Path $build 'classes') $sources
if ($LASTEXITCODE -ne 0) { throw 'javac failed' }

Write-Output '== jar + d8 =='
& (Join-Path $env:JAVA_HOME 'bin\jar.exe') cf (Join-Path $build 'classes.jar') `
    -C (Join-Path $build 'classes') .
if ($LASTEXITCODE -ne 0) { throw 'jar failed' }
New-Item -ItemType Directory -Force (Join-Path $build 'd8') | Out-Null
& (Join-Path $bt 'd8.bat') --lib $platform --min-api 26 `
    --output (Join-Path $build 'd8') (Join-Path $build 'classes.jar')
if ($LASTEXITCODE -ne 0) { throw 'd8 failed' }

Write-Output '== сборка APK (dex внутрь архива) =='
Copy-Item (Join-Path $build 'base.apk') (Join-Path $build 'unsigned.apk') -Force
& (Join-Path $env:JAVA_HOME 'bin\jar.exe') uf (Join-Path $build 'unsigned.apk') `
    -C (Join-Path $build 'd8') classes.dex
if ($LASTEXITCODE -ne 0) { throw 'jar (dex) failed' }

Write-Output '== zipalign =='
& (Join-Path $bt 'zipalign.exe') -f 4 (Join-Path $build 'unsigned.apk') (Join-Path $build 'aligned.apk')
if ($LASTEXITCODE -ne 0) { throw 'zipalign failed' }

Write-Output '== подпись (debug keystore) =='
$keystore = Join-Path $build 'debug.keystore'
if (-not (Test-Path $keystore)) {
    & (Join-Path $env:JAVA_HOME 'bin\keytool.exe') -genkeypair -keystore $keystore `
        -alias tensa -storepass tensa123 -keypass tensa123 `
        -keyalg RSA -keysize 2048 -validity 10000 -dname 'CN=Tensa'
    if ($LASTEXITCODE -ne 0) { throw 'keytool failed' }
}
& (Join-Path $bt 'apksigner.bat') sign --ks $keystore --ks-pass 'pass:tensa123' `
    --out $out (Join-Path $build 'aligned.apk')
if ($LASTEXITCODE -ne 0) { throw 'apksigner sign failed' }
& (Join-Path $bt 'apksigner.bat') verify $out
if ($LASTEXITCODE -ne 0) { throw 'apksigner verify failed' }

# Копия для скачивания с сервера ядра (PWA раздаёт pwa/static).
$repoRoot = Split-Path -Parent (Split-Path -Parent $here)
$staticDir = Join-Path $repoRoot 'pwa\static'
if (Test-Path $staticDir) {
    Copy-Item $out (Join-Path $staticDir 'Tensa.apk') -Force
    Write-Output "копия для телефона: $(Join-Path $staticDir 'Tensa.apk')"
}

Get-Item $out | ForEach-Object { Write-Output ("готово: {0} ({1:N0} байт)" -f $_.FullName, $_.Length) }
