; Compiled only by native_prerequisite_tests(); never a distribution artifact.
Unicode true
RequestExecutionLevel user
SilentInstall silent
Name "CodeConvoy prerequisite fixture"
OutFile "@OUTPUT@"
!addplugindir /x86-unicode "@HELPERS@"
!define CODECONVOY_CRT_FIXTURE
!define CODECONVOY_CRT_X86 "$EXEDIR\runtime-x86"
!define CODECONVOY_CRT_X64 "$EXEDIR\runtime-x64"
Var PassiveMode
!include "@PREREQUISITES@"

Function .onInit
  Call RequireWindowsRuntime
FunctionEnd

Function un.onInit
  Call un.RequireWindowsRuntime
FunctionEnd

!macro ExerciseHelper MARKER
  codeconvoy_utils::FindProcess "CodeConvoyMissingRuntimeFixture.exe"
  Pop $0
  ${If} $0 != "1"
    SetErrorLevel 40
    Quit
  ${EndIf}
  FileOpen $0 "$EXEDIR\${MARKER}" w
  FileWrite $0 "helper executed after prerequisites"
  FileClose $0
!macroend

Section
  !ifdef LOSE_RUNTIME
    ; Only an isolated copied fixture file, never a Windows system file.
    Delete "$EXEDIR\runtime-x86\vcruntime140.dll"
  !endif
  Call RequireWindowsRuntime
  !insertmacro ExerciseHelper "installer-helper.marker"
  WriteUninstaller "$EXEDIR\prerequisite-uninstall.exe"
  SetErrorLevel 0
SectionEnd

Section Uninstall
  Call un.RequireWindowsRuntime
  !insertmacro ExerciseHelper "uninstaller-helper.marker"
  SetErrorLevel 0
SectionEnd
