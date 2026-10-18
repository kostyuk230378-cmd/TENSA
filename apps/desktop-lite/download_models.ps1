# Скачивание моделей Tensa в %USERPROFILE%\.tensa\models ([derived]).
#
# Что кладёт:
#   1. GGUF локальной модели ядра — gemma-4-E2B-it-XS-TS-Q3_K_S.gguf
#      (~1.6 ГБ, TheStageAI/gemma-4-E2B-it-GGUF на HuggingFace).
#
# Эмбеддер памяти intfloat/multilingual-e5-small в установщик не входит:
# его скачивает fastembed сам (~120 МБ ONNX) при первом использовании памяти.
# Ссылка: https://huggingface.co/intfloat/multilingual-e5-small
#
# Использование:
#   -DryRun  — только проверить доступность и размеры (ничего не качать).

param(
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

$gemmaUrl = 'https://huggingface.co/TheStageAI/gemma-4-E2B-it-GGUF/resolve/main/gemma-4-E2B-it-XS-TS-Q3_K_S.gguf'
$gemmaFile = 'gemma-4-E2B-it-XS-TS-Q3_K_S.gguf'
# SHA256 файла (LFS oid TheStageAI/gemma-4-E2B-it-GGUF) — проверка после скачивания.
$gemmaSha256 = 'd1e358e0a9f945084e8757090684ef698f9e99f41693dcd465f5aeaa3564c3b5'

$models = Join-Path $env:USERPROFILE '.tensa\models'
New-Item -ItemType Directory -Force $models | Out-Null

function Get-Model {
    param([string]$Url, [string]$Name, [switch]$CheckOnly)

    $target = Join-Path $models $Name
    if (Test-Path $target) {
        Write-Output "  уже есть: $Name ($([math]::Round((Get-Item $target).Length / 1MB, 1)) МБ) — пропускаю"
        return
    }

    # Проверка доступности и размера (HEAD).
    $size = & curl.exe -sIL $Url | Select-String -Pattern '^content-length:' |
        Select-Object -Last 1
    $sizeText = if ($size) {
        $bytes = [int64]($size.Line -replace '[^0-9]', '')
        "$([math]::Round($bytes / 1MB, 1)) МБ"
    } else {
        'размер не определён'
    }
    Write-Output "  $Name — $sizeText"
    Write-Output "    $Url"

    if ($CheckOnly) {
        return
    }

    $tmp = "$target.part"
    Write-Output '    качаю (curl, с докачкой)...'
    & curl.exe -L --retry 3 -C - -o $tmp $Url
    if ($LASTEXITCODE -ne 0) {
        throw "не удалось скачать $Name (curl exit $LASTEXITCODE)"
    }
    $actual = (Get-FileHash $tmp -Algorithm SHA256).Hash.ToLower()
    if ($Name -eq $gemmaFile -and $actual -ne $gemmaSha256) {
        Remove-Item $tmp -Force
        throw "SHA256 не совпал для $Name (получено $actual) — скачивание отменено"
    }
    Move-Item $tmp $target -Force
    Write-Output "    готово: $target"
}

Write-Output '== Модели Tensa =='
Write-Output "каталог: $models"
$existing = Get-ChildItem $models -Filter *.gguf -ErrorAction SilentlyContinue
if ($existing) {
    Write-Output 'в каталоге уже есть модели:'
    $existing | ForEach-Object { Write-Output "  - $($_.Name) ($([math]::Round($_.Length / 1MB, 1)) МБ)" }
}
if ($DryRun) {
    Write-Output '(режим проверки: ничего не качается)'
}
Write-Output ''
Write-Output '1) Локальная модель ядра (GGUF, ~1.65 ГБ):'
Get-Model -Url $gemmaUrl -Name $gemmaFile -CheckOnly:$DryRun


Write-Output ''
Write-Output '2) Эмбеддер памяти intfloat/multilingual-e5-small:'
Write-Output '  скачивается fastembed автоматически (~120 МБ ONNX) при первом'
Write-Output '  использовании памяти; вручную: https://huggingface.co/intfloat/multilingual-e5-small'

Write-Output ''
Write-Output '3) Большие модели (облако): PWA → «Чат» или APK → «Чат»; ключ: https://openrouter.ai/keys'
