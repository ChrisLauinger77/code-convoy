; Compiled only by native_prerequisite_tests(); never a distribution artifact.
Unicode true
RequestExecutionLevel user
SilentInstall silent
Name "CodeConvoy prerequisite fixture"
OutFile "@OUTPUT@"
!include "@NATIVE@"
!define CODECONVOY_CRT_FIXTURE
!define CODECONVOY_CRT_X64 "$EXEDIR\runtime-x64"
Var PassiveMode
!include "@PREREQUISITES@"

Function .onInit
  Call RequireWindowsRuntime
FunctionEnd

!macro ExerciseNative PREFIX MARKER
  Push "CodeConvoyMissingRuntimeFixture.exe"
  Push 0
  Call ${PREFIX}CheckAppProcesses
  Pop $0
  ${If} $0 != "0"
    SetErrorLevel 40
    Quit
  ${EndIf}
  FileOpen $0 "$EXEDIR\${MARKER}" w
  FileWrite $0 "native process check executed"
  FileClose $0
!macroend

Section
  !ifdef LOSE_RUNTIME
    ; Only an isolated copied fixture file, never a Windows system file.
    Delete "$EXEDIR\runtime-x64\vcruntime140.dll"
  !endif
  Call RequireWindowsRuntime
  !insertmacro ExerciseNative "" "installer-native.marker"
  WriteUninstaller "$EXEDIR\prerequisite-uninstall.exe"
  SetErrorLevel 0
SectionEnd

Section Uninstall
  !insertmacro ExerciseNative "un." "uninstaller-native.marker"
  SetErrorLevel 0
SectionEnd
