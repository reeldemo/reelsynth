; ReelSynth NSIS installer — app + VST3 + Ableton editor/config
; Built by scripts/package-windows.ps1 (makensis)

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "x64.nsh"

!ifndef REELSYNTH_VERSION
  !define REELSYNTH_VERSION "0.0.0"
!endif
!ifndef REELSYNTH_STAGE
  !error "REELSYNTH_STAGE must be defined (path to staged payload)"
!endif
!ifndef REELSYNTH_OUT
  !error "REELSYNTH_OUT must be defined (output setup.exe path)"
!endif

Name "ReelSynth ${REELSYNTH_VERSION}"
OutFile "${REELSYNTH_OUT}"
Unicode true
SetCompressor /SOLID lzma
RequestExecutionLevel highest
InstallDir "$PROGRAMFILES64\ReelSynth"
InstallDirRegKey HKCU "Software\ReelSynth" "InstallDir"

Var VstDir
Var IsAdmin
Var EditorJsonPath

!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_TITLE "ReelSynth installed"
!define MUI_FINISHPAGE_TEXT "Quit Ableton Live if it was open, then Preferences → Plug-ins → Rescan VST3, and load ReelSynth on a MIDI track.$\r$\n$\r$\nUnsigned build: if SmartScreen warns, click More info → Run anyway."
!define MUI_FINISHPAGE_RUN "$INSTDIR\ReelSynth.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch ReelSynth"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  UserInfo::GetAccountType
  Pop $0
  ${If} $0 == "Admin"
    StrCpy $IsAdmin "1"
  ${Else}
    StrCpy $IsAdmin "0"
    StrCpy $INSTDIR "$LOCALAPPDATA\Programs\ReelSynth"
  ${EndIf}
FunctionEnd

Section "ReelSynth" SecMain
  SetOutPath "$INSTDIR"
  File "${REELSYNTH_STAGE}\ReelSynth.exe"
  File "${REELSYNTH_STAGE}\reelsynth-export.exe"
  File "${REELSYNTH_STAGE}\reelsynth-plugin-editor.exe"
  File "${REELSYNTH_STAGE}\reelsynth_plugin.dll"
  File /nonfatal "${REELSYNTH_STAGE}\LICENSE"
  File /nonfatal "${REELSYNTH_STAGE}\README.md"

  ; Editor + Ableton config (always per-user LocalAppData)
  CreateDirectory "$LOCALAPPDATA\ReelSynth\bin"
  CopyFiles /SILENT "$INSTDIR\reelsynth-plugin-editor.exe" "$LOCALAPPDATA\ReelSynth\bin\reelsynth-plugin-editor.exe"

  Push "$LOCALAPPDATA\ReelSynth\bin\reelsynth-plugin-editor.exe"
  Call StrRepSlash
  Pop $EditorJsonPath

  FileOpen $0 "$LOCALAPPDATA\ReelSynth\config.json" w
  FileWrite $0 '{"schema":"reelsynth-ableton-config-v1","auto_editor":true,"editor_path":"'
  FileWrite $0 $EditorJsonPath
  FileWrite $0 '"}'
  FileClose $0

  ; VST3 — system Common Files if admin, else Documents\VST3
  ${If} $IsAdmin == "1"
    StrCpy $VstDir "$COMMONFILES64\VST3\ReelSynth.vst3\Contents\x86_64-win"
  ${Else}
    StrCpy $VstDir "$PROFILE\Documents\VST3\ReelSynth.vst3\Contents\x86_64-win"
  ${EndIf}
  CreateDirectory "$VstDir"
  CopyFiles /SILENT "$INSTDIR\reelsynth_plugin.dll" "$VstDir\ReelSynth.vst3"
  CopyFiles /SILENT "$INSTDIR\reelsynth-plugin-editor.exe" "$VstDir\reelsynth-plugin-editor.exe"

  CreateDirectory "$SMPROGRAMS\ReelSynth"
  CreateShortCut "$SMPROGRAMS\ReelSynth\ReelSynth.lnk" "$INSTDIR\ReelSynth.exe"
  CreateShortCut "$SMPROGRAMS\ReelSynth\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
  CreateShortCut "$DESKTOP\ReelSynth.lnk" "$INSTDIR\ReelSynth.exe"

  WriteRegStr HKCU "Software\ReelSynth" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\ReelSynth" "VstDir" "$VstDir"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  DetailPrint "VST3 installed under: $VstDir"
  ${If} $IsAdmin != "1"
    DetailPrint "Non-admin: add Documents\VST3 in Live Preferences if needed, then Rescan."
  ${EndIf}
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\ReelSynth.exe"
  Delete "$INSTDIR\reelsynth-export.exe"
  Delete "$INSTDIR\reelsynth-plugin-editor.exe"
  Delete "$INSTDIR\reelsynth_plugin.dll"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\ReelSynth\ReelSynth.lnk"
  Delete "$SMPROGRAMS\ReelSynth\Uninstall.lnk"
  RMDir "$SMPROGRAMS\ReelSynth"
  Delete "$DESKTOP\ReelSynth.lnk"

  ReadRegStr $VstDir HKCU "Software\ReelSynth" "VstDir"
  ${If} $VstDir != ""
    Delete "$VstDir\ReelSynth.vst3"
    Delete "$VstDir\reelsynth-plugin-editor.exe"
  ${EndIf}

  DeleteRegKey HKCU "Software\ReelSynth"
SectionEnd

Function StrRepSlash
  Exch $R0
  Push $R1
  Push $R2
  StrCpy $R1 ""
  loop:
    StrCpy $R2 $R0 1
    StrCmp $R2 "" done
    StrCmp $R2 "\" is_slash
    StrCpy $R1 "$R1$R2"
    Goto next
  is_slash:
    StrCpy $R1 "$R1/"
  next:
    StrCpy $R0 $R0 "" 1
    Goto loop
  done:
    StrCpy $R0 $R1
    Pop $R2
    Pop $R1
    Exch $R0
FunctionEnd
