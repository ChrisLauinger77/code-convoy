; CodeConvoy: check actual system DLL versions and PE architectures, never registry
; claims alone. A stale or missing registry entry cannot override file evidence.
; The x86 NSIS host checks the x64 application runtime without loading it.
!include "LogicLib.nsh"
!include "x64.nsh"
!include "WinVer.nsh"

Var CrtFile
Var CrtMachine
Var CrtMinimumMS
Var CrtMinimumLS
Var CrtFileOK
Var CrtArchitectureOK
Var CrtDirectory
Var CrtMissing

; Test paths are compile-time substitutions in a separate fixture executable.
; Production configure() rejects these definitions; there is no runtime bypass.
!ifndef CODECONVOY_CRT_FIXTURE
  !define CODECONVOY_CRT_X64 "$WINDIR\Sysnative"
!endif

!macro CrtReadWord HANDLE VALUE TEMP
  FileReadByte ${HANDLE} ${VALUE}
  FileReadByte ${HANDLE} ${TEMP}
  IntOp ${TEMP} ${TEMP} << 8
  IntOp ${VALUE} ${VALUE} | ${TEMP}
!macroend

!macro CodeConvoyRuntimeFunctions PREFIX
Function ${PREFIX}CheckRuntimeFile
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  StrCpy $CrtFileOK 0
  ClearErrors
  GetDLLVersion "$CrtFile" $0 $1
  IfErrors done
  ; Unsigned comparison is essential: the build number occupies the high word.
  IntCmpU $0 $CrtMinimumMS compare_build done check_image
  compare_build:
    IntCmpU $1 $CrtMinimumLS check_image done check_image
  check_image:
    ClearErrors
    FileOpen $0 "$CrtFile" r
    IfErrors done
    !insertmacro CrtReadWord $0 $1 $2
    IfErrors close_file
    IntCmp $1 0x5a4d 0 close_file close_file
    FileSeek $0 0x3c SET
    !insertmacro CrtReadWord $0 $1 $2
    !insertmacro CrtReadWord $0 $3 $2
    IfErrors close_file
    IntOp $3 $3 << 16
    IntOp $1 $1 | $3
    FileSeek $0 $1 SET
    !insertmacro CrtReadWord $0 $1 $2
    IfErrors close_file
    IntCmp $1 0x4550 0 close_file close_file
    !insertmacro CrtReadWord $0 $1 $2
    IfErrors close_file
    IntCmp $1 0 0 close_file close_file
    !insertmacro CrtReadWord $0 $1 $2
    IfErrors close_file
    IntCmp $1 $CrtMachine 0 close_file close_file
    StrCpy $CrtFileOK 1
  close_file:
    FileClose $0
  done:
    Pop $4
    Pop $3
    Pop $2
    Pop $1
    Pop $0
FunctionEnd

Function ${PREFIX}CheckWindowsRuntime
  StrCpy $CrtMissing ""
  ${IfNot} ${AtLeastWin10}
  ${OrIfNot} ${RunningX64}
    StrCpy $CrtMissing "A supported x64 Windows 10/11 system is required."
    Return
  ${EndIf}
  StrCpy $CrtDirectory "${CODECONVOY_CRT_X64}"
  StrCpy $CrtMachine 0x8664
  StrCpy $CrtArchitectureOK 1
@CHECKS_X64@
  ${If} $CrtArchitectureOK != 1
    StrCpy $CrtMissing "x64 runtime (required by CodeConvoy)"
  ${EndIf}
FunctionEnd

Function ${PREFIX}RequireWindowsRuntime
  Call ${PREFIX}CheckWindowsRuntime
  ${If} $CrtMissing != ""
    StrCpy $CrtMissing "Missing, outdated or unusable Microsoft runtime:$\r$\n$CrtMissing$\r$\n$\r$\nInstall or repair the latest Microsoft Visual C++ v14 Redistributable for the architecture(s) above (VCRUNTIME140.dll @VC_VERSION@ or newer), then retry. Keep Windows updated for the Universal CRT.$\r$\n$\r$\n@DOWNLOAD_PAGE@$\r$\n$\r$\nCodeConvoy does not download or install these prerequisites."
    ; Also works with redirected stderr; no console is created for GUI users.
    System::Call 'kernel32::AttachConsole(i -1)'
    System::Call 'kernel32::GetStdHandle(i -12)p.r0'
    FileWrite $0 "$CrtMissing$\r$\n"
    IfSilent no_dialog
    ${If} $PassiveMode != 1
      MessageBox MB_OK|MB_ICONSTOP "$CrtMissing"
    ${EndIf}
    no_dialog:
      SetErrorLevel 1603
      Quit
  ${EndIf}
FunctionEnd
!macroend

!insertmacro CodeConvoyRuntimeFunctions ""
; New uninstall uses only Windows APIs. A released 1.0.0 uninstaller retains
; its own embedded x86 runtime check when invoked by the maintenance page.
