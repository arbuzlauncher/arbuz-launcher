Unicode true
SetCompressor /SOLID lzma
ManifestDPIAware true

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "nsDialogs.nsh"

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
; Установщик не просит права администратора при запуске. Режим установки
; выбирается в конце мастера: «для всех» перезапускает установщик через UAC
; (ExecShell runas), «для одного» ставится сразу без повышения прав.
; ---------------------------------------------------------------------------
RequestExecutionLevel user

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

!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfElevated
!insertmacro MUI_PAGE_WELCOME
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfElevated
!insertmacro MUI_PAGE_DIRECTORY
Page custom ShortcutsPageCreate ShortcutsPageLeave
Page custom ModePageCreate ModePageLeave
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "$(RunApp)"
!define MUI_FINISHPAGE_RUN_FUNCTION RunLauncher
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
LangString StartMenuShortcut ${LANG_ENGLISH} "Create a Start Menu shortcut"
LangString StartMenuShortcut ${LANG_RUSSIAN} "Создать ярлык в меню «Пуск»"
LangString ShortcutsTitle ${LANG_ENGLISH} "Shortcuts"
LangString ShortcutsTitle ${LANG_RUSSIAN} "Ярлыки"
LangString ShortcutsSubtitle ${LANG_ENGLISH} "Choose which shortcuts to create."
LangString ShortcutsSubtitle ${LANG_RUSSIAN} "Выберите, какие ярлыки создать."
LangString AskData ${LANG_ENGLISH} "Also delete settings, profiles, worlds and accounts?"
LangString AskData ${LANG_RUSSIAN} "Удалить также настройки, профили, миры и аккаунты?"
LangString Busy ${LANG_ENGLISH} "The launcher is running. Close it and try again."
LangString Busy ${LANG_RUSSIAN} "Лаунчер запущен. Закройте его и повторите."
LangString AlreadyInstalled ${LANG_ENGLISH} "${APP_NAME} is already installed:$\r$\n$ExistingDir$\r$\n$\r$\nYes — reinstall and keep settings.$\r$\nNo — uninstall.$\r$\nCancel — exit."
LangString AlreadyInstalled ${LANG_RUSSIAN} "${APP_NAME} уже установлен:$\r$\n$ExistingDir$\r$\n$\r$\nДа — переустановить, настройки сохранятся.$\r$\nНет — удалить.$\r$\nОтмена — выйти."
LangString NeedAdmin ${LANG_ENGLISH} "This version was installed for all users. Run the installer as administrator to remove it."
LangString NeedAdmin ${LANG_RUSSIAN} "Эта версия установлена для всех пользователей. Запустите установщик от имени администратора, чтобы удалить её."
LangString NeedAdminInstall ${LANG_ENGLISH} "Administrator rights are required to install for all users."
LangString NeedAdminInstall ${LANG_RUSSIAN} "Для установки для всех пользователей нужны права администратора."

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

Var InstallMode
Var IsAdmin
Var Elevated
Var StartMenuChk
Var DesktopChk
Var ExistingUninstall
Var ExistingDir
Var ExistingMode
Var SmCheckbox
Var DtCheckbox
Var ModeRadioAll
Var ModeRadioCur

; Права администратора есть только у действительно повышенного процесса:
; проверяем записью в системную папку (у обычного пользователя её нет).
Function DetectElevated
  StrCpy $IsAdmin 0
  ClearErrors
  FileOpen $0 "$WINDIR\arbuz-elev-test.tmp" w
  IfErrors done
  FileClose $0
  Delete "$WINDIR\arbuz-elev-test.tmp"
  StrCpy $IsAdmin 1
  done:
FunctionEnd

Function un.DetectElevated
  StrCpy $IsAdmin 0
  ClearErrors
  FileOpen $0 "$WINDIR\arbuz-elev-test.tmp" w
  IfErrors done
  FileClose $0
  Delete "$WINDIR\arbuz-elev-test.tmp"
  StrCpy $IsAdmin 1
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

Function SkipIfElevated
  ${If} $Elevated == 1
    Abort
  ${EndIf}
FunctionEnd

Function .onInit
  SetRegView 64
  Call DetectElevated

  StrCpy $InstallMode "CurrentUser"
  StrCpy $Elevated 0
  StrCpy $StartMenuChk 1
  StrCpy $DesktopChk 0

  ; Перезапуск с правами администратора: язык, папка и ярлыки приходят
  ; из командной строки, мастер сразу переходит к установке.
  ${GetOptions} $CMDLINE "/elevated" $0
  ${IfNot} ${Errors}
    StrCpy $Elevated 1
  ${EndIf}
  ${GetOptions} $CMDLINE "/allusers" $0
  ${IfNot} ${Errors}
    StrCpy $InstallMode "AllUsers"
  ${EndIf}
  ${GetOptions} $CMDLINE "/nostartmenu" $0
  ${IfNot} ${Errors}
    StrCpy $StartMenuChk 0
  ${EndIf}
  ${GetOptions} $CMDLINE "/nodesktop" $0
  ${IfNot} ${Errors}
    StrCpy $DesktopChk 0
  ${EndIf}
  ${GetOptions} $CMDLINE "/desktop" $0
  ${IfNot} ${Errors}
    StrCpy $DesktopChk 1
  ${EndIf}

  ${If} $Elevated == 1
    ${GetOptions} $CMDLINE "/LANG=" $0
    ${IfNot} ${Errors}
      StrCpy $LANGUAGE $0
    ${EndIf}
    ${If} $IsAdmin != 1
      MessageBox MB_OK|MB_ICONSTOP "$(NeedAdminInstall)"
      Quit
    ${EndIf}
  ${Else}
    !insertmacro MUI_LANGDLL_DISPLAY
  ${EndIf}

  ${If} $Elevated != 1
    Call DetectExisting
    ; Тихая установка всегда обновляет существующую копию, не показывая вопрос.
    IfSilent 0 ask_existing
      ${If} $ExistingMode == "AllUsers"
        StrCpy $InstallMode "AllUsers"
        StrCpy $INSTDIR $ExistingDir
      ${EndIf}
      Goto no_existing
    ask_existing:
    StrCmp $ExistingUninstall "" no_existing

    ; Существующая установка задаёт режим и папку по умолчанию.
    StrCmp $ExistingMode "AllUsers" 0 ask
    StrCpy $InstallMode "AllUsers"
    StrCpy $INSTDIR $ExistingDir

    ask:
    MessageBox MB_YESNOCANCEL|MB_ICONQUESTION "$(AlreadyInstalled)" IDYES reinstall IDNO uninstall
    Quit

    uninstall:
      ExecWait '$ExistingUninstall'
      Quit

    reinstall:
    no_existing:

    ; Каталог по умолчанию: Program Files для «всех», профиль — для одного.
    ; Если /D= уже задал другую папку, не перезаписываем её.
    ${If} $INSTDIR == "$PROFILE\arbuz-launcher"
      ${If} $InstallMode == "AllUsers"
        ReadRegStr $0 HKLM "${APP_KEY}" "InstallDir"
        ${If} $0 == ""
          StrCpy $INSTDIR "$PROGRAMFILES64\Arbuz Launcher"
        ${Else}
          StrCpy $INSTDIR $0
        ${EndIf}
      ${Else}
        ReadRegStr $0 HKCU "${APP_KEY}" "InstallDir"
        ${If} $0 == ""
          StrCpy $INSTDIR "$PROFILE\arbuz-launcher"
        ${Else}
          StrCpy $INSTDIR $0
        ${EndIf}
      ${EndIf}
    ${EndIf}
  ${EndIf}
FunctionEnd

Function un.onInit
  SetRegView 64
  Call un.DetectElevated
  !insertmacro MUI_UNGETLANGUAGE

  ReadRegStr $0 HKLM "${UNINST_KEY}" "InstallMode"
  ${If} $0 == "AllUsers"
    StrCpy $InstallMode "AllUsers"
    ReadRegStr $INSTDIR HKLM "${APP_KEY}" "InstallDir"
  ${Else}
    ReadRegStr $0 HKCU "${UNINST_KEY}" "InstallMode"
    ${If} $0 == "AllUsers"
      StrCpy $InstallMode "AllUsers"
      ReadRegStr $INSTDIR HKCU "${APP_KEY}" "InstallDir"
    ${Else}
      StrCpy $InstallMode "CurrentUser"
      ReadRegStr $INSTDIR HKCU "${APP_KEY}" "InstallDir"
    ${EndIf}
  ${EndIf}

  ${If} $InstallMode == "AllUsers"
    ${If} $IsAdmin != 1
      ClearErrors
      ExecShell "runas" "$INSTDIR\uninstall.exe" "/elevated"
      ${If} ${Errors}
        MessageBox MB_OK|MB_ICONSTOP "$(NeedAdmin)"
      ${EndIf}
      Quit
    ${EndIf}
    SetShellVarContext all
  ${Else}
    SetShellVarContext current
  ${EndIf}
FunctionEnd

Function ShortcutsPageCreate
  ${If} $Elevated == 1
    Abort
  ${EndIf}
  !insertmacro MUI_HEADER_TEXT "$(ShortcutsTitle)" "$(ShortcutsSubtitle)"
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}

  ${NSD_CreateCheckbox} 0 0 100% 12u "$(StartMenuShortcut)"
  Pop $SmCheckbox
  ${If} $StartMenuChk == 1
    ${NSD_SetState} $SmCheckbox ${BST_CHECKED}
  ${EndIf}

  ${NSD_CreateCheckbox} 0 20u 100% 12u "$(Desktop)"
  Pop $DtCheckbox
  ${If} $DesktopChk == 1
    ${NSD_SetState} $DtCheckbox ${BST_CHECKED}
  ${EndIf}

  nsDialogs::Show
FunctionEnd

Function ShortcutsPageLeave
  ${NSD_GetState} $SmCheckbox $0
  ${If} $0 == ${BST_CHECKED}
    StrCpy $StartMenuChk 1
  ${Else}
    StrCpy $StartMenuChk 0
  ${EndIf}
  ${NSD_GetState} $DtCheckbox $0
  ${If} $0 == ${BST_CHECKED}
    StrCpy $DesktopChk 1
  ${Else}
    StrCpy $DesktopChk 0
  ${EndIf}
FunctionEnd

Function ModePageCreate
  ${If} $Elevated == 1
    Abort
  ${EndIf}
  !insertmacro MUI_HEADER_TEXT "$(MULTIUSER_TEXT_INSTALLMODE_TITLE)" "$(MULTIUSER_TEXT_INSTALLMODE_SUBTITLE)"
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}

  ${NSD_CreateLabel} 0 0 100% 40u "$(MULTIUSER_INNERTEXT_INSTALLMODE_TOP)"
  Pop $0
  ${NSD_CreateRadioButton} 15u 50u -15u 12u "$(MULTIUSER_INNERTEXT_INSTALLMODE_ALLUSERS)"
  Pop $ModeRadioAll
  ${NSD_CreateRadioButton} 15u 72u -15u 12u "$(MULTIUSER_INNERTEXT_INSTALLMODE_CURRENTUSER)"
  Pop $ModeRadioCur
  ${If} $InstallMode == "AllUsers"
    ${NSD_SetState} $ModeRadioAll ${BST_CHECKED}
  ${Else}
    ${NSD_SetState} $ModeRadioCur ${BST_CHECKED}
  ${EndIf}

  nsDialogs::Show
FunctionEnd

Function ModePageLeave
  ${NSD_GetState} $ModeRadioAll $0
  ${If} $0 == ${BST_CHECKED}
    StrCpy $InstallMode "AllUsers"
    ; Папку не меняли — для общей установки берём Program Files.
    ${If} $INSTDIR == "$PROFILE\arbuz-launcher"
      StrCpy $INSTDIR "$PROGRAMFILES64\Arbuz Launcher"
    ${EndIf}
    ${If} $IsAdmin != 1
      StrCpy $1 ""
      ${If} $StartMenuChk != 1
        StrCpy $1 "$1 /nostartmenu"
      ${EndIf}
      ${If} $DesktopChk == 1
        StrCpy $1 "$1 /desktop"
      ${Else}
        StrCpy $1 "$1 /nodesktop"
      ${EndIf}
      ClearErrors
      ExecShell "runas" "$EXEPATH" "/allusers /elevated /LANG=$LANGUAGE$1 /D=$INSTDIR"
      ${If} ${Errors}
        MessageBox MB_OK|MB_ICONSTOP "$(NeedAdminInstall)"
        Abort
      ${EndIf}
      Quit
    ${EndIf}
  ${Else}
    StrCpy $InstallMode "CurrentUser"
  ${EndIf}
FunctionEnd

Function RunLauncher
  ${If} $Elevated == 1
    ; Из повышенного процесса запускаем через Explorer, чтобы лаунчер
    ; стартовал от обычного пользователя, а не от администратора.
    ExecShell "" "explorer.exe" '"$INSTDIR\${APP_EXE}"'
  ${Else}
    ExecShell "open" "$INSTDIR\${APP_EXE}"
  ${EndIf}
FunctionEnd

Section "Install"
  ${If} $InstallMode == "AllUsers"
  ${AndIf} $IsAdmin != 1
    MessageBox MB_ICONSTOP "$(NeedAdminInstall)"
    Abort
  ${EndIf}

  SetShellVarContext current
  ${If} $InstallMode == "AllUsers"
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

  ${If} $StartMenuChk == 1
    CreateDirectory "$SMPROGRAMS\${APP_NAME}"
    CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
  ${EndIf}
  ${If} $DesktopChk == 1
    CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
  ${EndIf}

  WriteRegStr SHCTX "${APP_KEY}" "InstallDir" "$INSTDIR"
  WriteRegStr SHCTX "${APP_KEY}" "InstallMode" "$InstallMode"
  WriteRegStr SHCTX "${UNINST_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr SHCTX "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr SHCTX "${UNINST_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr SHCTX "${UNINST_KEY}" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr SHCTX "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr SHCTX "${UNINST_KEY}" "InstallMode" "$InstallMode"
  WriteRegStr SHCTX "${UNINST_KEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegStr SHCTX "${UNINST_KEY}" "QuietUninstallString" "$\"$INSTDIR\uninstall.exe$\" /S"
  WriteRegDWORD SHCTX "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD SHCTX "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD SHCTX "${UNINST_KEY}" "EstimatedSize" "$0"
SectionEnd

Section "Uninstall"
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
