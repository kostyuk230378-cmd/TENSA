; NSIS-скрипт установщика Tensa для Windows ([derived]: тестовый дистрибутив —
; ядро + PWA + CLI + бот; полноценное десктоп-приложение из ТЗ apps/desktop —
; отдельный этап).
;
; Сборка: powershell -ExecutionPolicy Bypass -File apps/desktop-lite/build_installer.ps1
; Результат: apps/desktop-lite/TensaSetup.exe

Unicode true

!include "MUI2.nsh"

!define APPNAME "Tensa"
!define APPVERSION "0.1.0"

!ifndef STAGE
  !define STAGE "C:\tensa-build\stage"
!endif

Name "${APPNAME} ${APPVERSION}"
OutFile "TensaSetup.exe"
InstallDir "$LOCALAPPDATA\Tensa"
InstallDirRegKey HKCU "Software\Tensa" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define MUI_ICON "${STAGE}\tensa.ico"
!define MUI_UNICON "${STAGE}\tensa.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Russian"
!insertmacro MUI_LANGUAGE "English"

Section "Tensa" SecMain
  SetOutPath "$INSTDIR"
  File "${STAGE}\tensa-server.exe"
  File "${STAGE}\tensa-cli.exe"
  File "${STAGE}\tensa-bot.exe"
  File "${STAGE}\tensa-desktop.exe"
  File "${STAGE}\tensa-whisper.exe"
  File "${STAGE}\tensa.ico"
  File "${STAGE}\Tensa.cmd"
  File "${STAGE}\TensaBot.cmd"
  File "${STAGE}\README.txt"
  File "${STAGE}\download_models.ps1"
  File "${STAGE}\DownloadModels.cmd"

  SetOutPath "$INSTDIR\models"
  File /r "${STAGE}\models\*.*"

  SetOutPath "$INSTDIR\pwa"
  File /r "${STAGE}\pwa\*.*"

  WriteRegStr HKCU "Software\Tensa" "InstallDir" "$INSTDIR"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  CreateShortCut "$DESKTOP\Tensa.lnk" "$INSTDIR\tensa-desktop.exe" "" "$INSTDIR\tensa.ico" 0
  CreateDirectory "$SMPROGRAMS\Tensa"
  CreateShortCut "$SMPROGRAMS\Tensa\Tensa.lnk" "$INSTDIR\tensa-desktop.exe" "" "$INSTDIR\tensa.ico" 0
  CreateShortCut "$SMPROGRAMS\Tensa\Tensa (core console).lnk" "$INSTDIR\Tensa.cmd" "" "$INSTDIR\tensa.ico" 0
  CreateShortCut "$SMPROGRAMS\Tensa\Tensa (Telegram bot).lnk" "$INSTDIR\TensaBot.cmd" "" "$INSTDIR\tensa.ico" 0
  CreateShortCut "$SMPROGRAMS\Tensa\Tensa (download models).lnk" "$INSTDIR\DownloadModels.cmd" "" "$INSTDIR\tensa.ico" 0
  CreateShortCut "$SMPROGRAMS\Tensa\Uninstall Tensa.lnk" "$INSTDIR\Uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\tensa-server.exe"
  Delete "$INSTDIR\tensa-cli.exe"
  Delete "$INSTDIR\tensa-bot.exe"
  Delete "$INSTDIR\tensa-desktop.exe"
  Delete "$INSTDIR\tensa-whisper.exe"
  Delete "$INSTDIR\tensa.ico"
  Delete "$INSTDIR\Tensa.cmd"
  Delete "$INSTDIR\TensaBot.cmd"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\download_models.ps1"
  Delete "$INSTDIR\DownloadModels.cmd"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir /r "$INSTDIR\pwa"
  RMDir /r "$INSTDIR\models"
  RMDir /r "$INSTDIR\tensa-desktop.exe.WebView2"
  RMDir "$INSTDIR"

  Delete "$DESKTOP\Tensa.lnk"
  RMDir /r "$SMPROGRAMS\Tensa"
  DeleteRegKey HKCU "Software\Tensa"
SectionEnd
