Unicode true
SetCompressor /SOLID lzma
ManifestDPIAware true

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"

!ifndef ROOT
  !define ROOT ".."
!endif
!ifndef APP_VERSION
  !define APP_VERSION "1.0.0"
!endif
!ifndef APP_VER4
  !define APP_VER4 "1.0.0.0"
!endif

!define APP_NAME "Arbuz Launcher"
!define APP_EXE "arbuz-launcher.exe"
!define APP_PUBLISHER "Arbuz"
!define APP_DATA "$PROFILE\arbuz-launcher"
!define APP_DATA_BESIDE "$INSTDIR\arbuz-launcher"
!define APP_DATA_OLD "$APPDATA\Arbuz Launcher"
!define APP_KEY "Software\${APP_PUBLISHER}\${APP_NAME}"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"

; ---------------------------------------------------------------------------
; Установка для текущего пользователя или для всех (с правами администратора).
; MultiUser сам показывает выбор режима и запрашивает UAC через «highest».
; ---------------------------------------------------------------------------
!define MULTIUSER_EXECUTIONLEVEL Highest
!define MULTIUSER_MUI
!define MULTIUSER_INSTALLMODE_COMMANDLINE
!define MULTIUSER_INSTALLMODE_DEFAULT_CURRENTUSER
!define MULTIUSER_USE_PROGRAMFILES64
!define MULTIUSER_INSTALLMODE_INSTDIR "Arbuz Launcher"
!define MULTIUSER_INSTALLMODE_INSTDIR_REGISTRY_KEY "${APP_KEY}"
!define MULTIUSER_INSTALLMODE_INSTDIR_REGISTRY_VALUENAME "InstallDir"
!define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_KEY "${APP_KEY}"
!define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_VALUENAME "InstallMode"
!define MULTIUSER_INSTALLMODE_FUNCTION SetInstallDir
!include "MultiUser.nsh"

Name "${APP_NAME}"
OutFile "${ROOT}\src-tauri\target\release\bundle\arbuz\Arbuz-Launcher-${APP_VERSION}-setup.exe"
InstallDir "$PROFILE\arbuz-launcher"
ShowInstDetails show
ShowUninstDetails show

VIProductVersion "${APP_VER4}"
VIAddVersionKey /LANG=1033 "ProductName" "${APP_NAME}"
VIAddVersionKey /LANG=1033 "FileDescription" "${APP_NAME} Setup"
VIAddVersionKey /LANG=1033 "FileVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "ProductVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "CompanyName" "${APP_PUBLISHER}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "${APP_PUBLISHER}"

!define MUI_ICON "icon.ico"
!define MUI_UNICON "icon.ico"
!define MUI_BGCOLOR "11140F"
!define MUI_TEXTCOLOR "E1E4DA"
!define MUI_INSTFILESPAGE_COLORS "E1E4DA 11140F"
!define MUI_INSTFILESPAGE_PROGRESSBAR "colored"
!define MUI_ABORTWARNING
!define MUI_LANGDLL_REGISTRY_ROOT "HKCU"
!define MUI_LANGDLL_REGISTRY_KEY "${APP_KEY}"
!define MUI_LANGDLL_REGISTRY_VALUENAME "Language"

!insertmacro MULTIUSER_PAGE_INSTALLMODE
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "$(RunApp)"
!define MUI_FINISHPAGE_SHOWREADME
!define MUI_FINISHPAGE_SHOWREADME_TEXT "$(Desktop)"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION CreateDesktopShortcut
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Russian"
!insertmacro MUI_LANGUAGE "Polish"
!insertmacro MUI_LANGUAGE "Ukrainian"
!insertmacro MUI_LANGUAGE "Belarusian"
!insertmacro MUI_LANGUAGE "Serbian"
!insertmacro MUI_LANGUAGE "Slovak"
!insertmacro MUI_LANGUAGE "Esperanto"
!insertmacro MUI_LANGUAGE "Estonian"
!insertmacro MUI_LANGUAGE "Finnish"
!insertmacro MUI_LANGUAGE "French"
!insertmacro MUI_LANGUAGE "German"
!insertmacro MUI_LANGUAGE "Latvian"
!insertmacro MUI_LANGUAGE "Lithuanian"
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "Japanese"
!insertmacro MUI_LANGUAGE "Uzbek"
!insertmacro MUI_LANGUAGE "Georgian"
!insertmacro MUI_LANGUAGE "Kazakh"
!insertmacro MUI_LANGUAGE "Kyrgyz"
!insertmacro MUI_LANGUAGE "Tajik"
!insertmacro MUI_LANGUAGE "Inuktitut"
!insertmacro MUI_LANGUAGE "Farsi"
!insertmacro MUI_LANGUAGE "Arabic"
!insertmacro MUI_LANGUAGE "Hebrew"
!insertmacro MUI_LANGUAGE "Armenian"
!insertmacro MUI_LANGUAGE "Portuguese"
!insertmacro MUI_LANGUAGE "Azerbaijani"
!insertmacro MUI_LANGUAGE "Olbanian"
!insertmacro MUI_RESERVEFILE_LANGDLL

!include "lang\custom.nsh"
!include "lang\strings.nsh"

LangString RunApp ${LANG_ENGLISH} "Run ${APP_NAME}"
LangString RunApp ${LANG_RUSSIAN} "Запустить ${APP_NAME}"
LangString Desktop ${LANG_ENGLISH} "Create desktop shortcut"
LangString Desktop ${LANG_RUSSIAN} "Создать ярлык на рабочем столе"
LangString AskData ${LANG_ENGLISH} "Also delete settings, profiles, worlds and accounts?"
LangString AskData ${LANG_RUSSIAN} "Удалить также настройки, профили, миры и аккаунты?"
LangString Busy ${LANG_ENGLISH} "The launcher is running. Close it and try again."
LangString Busy ${LANG_RUSSIAN} "Лаунчер запущен. Закройте его и повторите."
LangString AlreadyInstalled ${LANG_ENGLISH} "${APP_NAME} is already installed:$\r$\n$ExistingDir$\r$\n$\r$\nYes — reinstall and keep settings.$\r$\nNo — uninstall.$\r$\nCancel — exit."
LangString AlreadyInstalled ${LANG_RUSSIAN} "${APP_NAME} уже установлен:$\r$\n$ExistingDir$\r$\n$\r$\nДа — переустановить, настройки сохранятся.$\r$\nНет — удалить.$\r$\nОтмена — выйти."
LangString NeedAdmin ${LANG_ENGLISH} "This version was installed for all users. Run the installer as administrator to remove it."
LangString NeedAdmin ${LANG_RUSSIAN} "Эта версия установлена для всех пользователей. Запустите установщик от имени администратора, чтобы удалить её."

; Режим установки: по умолчанию — для одного пользователя (рекомендуется).
LangString MULTIUSER_TEXT_INSTALLMODE_TITLE ${LANG_ENGLISH} "Installation mode"
LangString MULTIUSER_TEXT_INSTALLMODE_TITLE ${LANG_RUSSIAN} "Режим установки"
LangString MULTIUSER_TEXT_INSTALLMODE_SUBTITLE ${LANG_ENGLISH} "Choose how to install ${APP_NAME}: for all users or for a single user."
LangString MULTIUSER_TEXT_INSTALLMODE_SUBTITLE ${LANG_RUSSIAN} "Выберите, как установить ${APP_NAME}: для всех пользователей или для одного."
LangString MULTIUSER_INNERTEXT_INSTALLMODE_TOP ${LANG_ENGLISH} "Select the installation mode:"
LangString MULTIUSER_INNERTEXT_INSTALLMODE_TOP ${LANG_RUSSIAN} "Выберите режим установки:"
LangString MULTIUSER_INNERTEXT_INSTALLMODE_ALLUSERS ${LANG_ENGLISH} "Install for all users (administrator rights)"
LangString MULTIUSER_INNERTEXT_INSTALLMODE_ALLUSERS ${LANG_RUSSIAN} "Установить для всех пользователей (права администратора)"
LangString MULTIUSER_INNERTEXT_INSTALLMODE_CURRENTUSER ${LANG_ENGLISH} "Install for a single user (recommended)"
LangString MULTIUSER_INNERTEXT_INSTALLMODE_CURRENTUSER ${LANG_RUSSIAN} "Установить для одного пользователя (рекомендуется)"

; Для олбанского — короткий английский вариант вместо пустого окна.
!macro INSTALLER_FALLBACKS LANGID
  LangString AlreadyInstalled ${LANGID} "${APP_NAME} is already installed in $ExistingDir. Yes — reinstall, No — uninstall, Cancel — exit."
  LangString NeedAdmin ${LANGID} "This version was installed for all users. Run the installer as administrator to remove it."
!macroend
!insertmacro INSTALLER_FALLBACKS ${LANG_OLBANIAN}

Var ExistingUninstall
Var ExistingDir
Var ExistingMode

; Каталог по умолчанию: Program Files для «всех», профиль — для одного.
; Если лаунчер уже стоял, берём InstallDir из реестра.
Function SetInstallDir
  ${If} $MultiUser.InstallMode == "AllUsers"
    ReadRegStr $0 HKLM "${APP_KEY}" "InstallDir"
    StrCmp $0 "" 0 existing
    StrCpy $INSTDIR "$PROGRAMFILES64\Arbuz Launcher"
    Goto done
  ${Else}
    ReadRegStr $0 HKCU "${APP_KEY}" "InstallDir"
    StrCmp $0 "" 0 existing
    StrCpy $INSTDIR "$PROFILE\arbuz-launcher"
    Goto done
  ${EndIf}
  existing:
  StrCpy $INSTDIR $0
  done:
FunctionEnd

; Для деинсталлятора каталог всегда берём из реестра выбранного режима.
Function un.SetUninstallDir
  ${If} $MultiUser.InstallMode == "AllUsers"
    ReadRegStr $0 HKLM "${APP_KEY}" "InstallDir"
  ${Else}
    ReadRegStr $0 HKCU "${APP_KEY}" "InstallDir"
  ${EndIf}
  StrCmp $0 "" done
    StrCpy $INSTDIR $0
  done:
FunctionEnd

; Ищет уже установленную копию: сначала общую (HKLM), потом пользовательскую.
Function DetectExisting
  StrCpy $ExistingUninstall ""
  StrCpy $ExistingDir ""
  StrCpy $ExistingMode ""
  ReadRegStr $0 HKLM "${UNINST_KEY}" "UninstallString"
  ReadRegStr $1 HKLM "${UNINST_KEY}" "InstallLocation"
  StrCmp $0 "" check_user
  StrCpy $ExistingUninstall $0
  StrCpy $ExistingDir $1
  StrCpy $ExistingMode "AllUsers"
  Goto done
  check_user:
  ReadRegStr $0 HKCU "${UNINST_KEY}" "UninstallString"
  ReadRegStr $1 HKCU "${UNINST_KEY}" "InstallLocation"
  StrCmp $0 "" done
  StrCpy $ExistingUninstall $0
  StrCpy $ExistingDir $1
  StrCpy $ExistingMode "CurrentUser"
  done:
FunctionEnd

Function .onInit
  SetRegView 64
  !insertmacro MUI_LANGDLL_DISPLAY
  !insertmacro MULTIUSER_INIT
  Call DetectExisting
  ; Тихая установка всегда обновляет существующую копию, не показывая вопрос.
  IfSilent no_existing
  StrCmp $ExistingUninstall "" no_existing

  ; Общую установку без прав администратора удалить нельзя.
  StrCmp $ExistingMode "AllUsers" 0 ask
  ${If} $MultiUser.Privileges != "Admin"
  ${AndIf} $MultiUser.Privileges != "Power"
    MessageBox MB_OK|MB_ICONSTOP "$(NeedAdmin)"
    Abort
  ${EndIf}

  ask:
  MessageBox MB_YESNOCANCEL|MB_ICONQUESTION "$(AlreadyInstalled)" IDYES reinstall IDNO uninstall
  Quit

  uninstall:
    ExecWait '$ExistingUninstall'
    Quit

  reinstall:
  no_existing:
FunctionEnd

Function un.onInit
  SetRegView 64
  !insertmacro MUI_UNGETLANGUAGE
  !insertmacro MULTIUSER_UNINIT
  Call un.SetUninstallDir
FunctionEnd

Section "Install"
  SetShellVarContext current
  ${If} $MultiUser.InstallMode == "AllUsers"
    SetShellVarContext all
  ${EndIf}

  SetOutPath $INSTDIR

  IfFileExists "$INSTDIR\${APP_EXE}" 0 files_free
    Delete "$INSTDIR\${APP_EXE}"
    IfFileExists "$INSTDIR\${APP_EXE}" 0 files_free
      MessageBox MB_ICONSTOP "$(Busy)"
      Abort
  files_free:

  File "/oname=${APP_EXE}" "${ROOT}\src-tauri\target\release\${APP_EXE}"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"

  WriteRegStr SHCTX "${APP_KEY}" "InstallDir" "$INSTDIR"
  WriteRegStr SHCTX "${APP_KEY}" "InstallMode" "$MultiUser.InstallMode"
  WriteRegStr SHCTX "${UNINST_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr SHCTX "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr SHCTX "${UNINST_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr SHCTX "${UNINST_KEY}" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr SHCTX "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr SHCTX "${UNINST_KEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegStr SHCTX "${UNINST_KEY}" "QuietUninstallString" "$\"$INSTDIR\uninstall.exe$\" /S"
  WriteRegDWORD SHCTX "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD SHCTX "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD SHCTX "${UNINST_KEY}" "EstimatedSize" "$0"
SectionEnd

Function CreateDesktopShortcut
  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
FunctionEnd

Section "Uninstall"
  SetShellVarContext current
  ${If} $MultiUser.InstallMode == "AllUsers"
    SetShellVarContext all
  ${EndIf}

  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"
  Delete "$DESKTOP\${APP_NAME}.lnk"

  DeleteRegKey SHCTX "${UNINST_KEY}"
  DeleteRegKey SHCTX "${APP_KEY}"
  DeleteRegKey /ifempty SHCTX "Software\${APP_PUBLISHER}"

  IfSilent keep
  MessageBox MB_YESNO|MB_ICONQUESTION "$(AskData)" IDNO keep
    ; Данные лежат рядом с exe, в профиле пользователя и в старом AppData.
    RMDir /r "${APP_DATA_BESIDE}"
    RMDir /r "${APP_DATA}"
    RMDir /r "${APP_DATA_OLD}"
    StrCmp "$INSTDIR" "${APP_DATA}" 0 keep
      RMDir /r "$INSTDIR"
  keep:
SectionEnd
