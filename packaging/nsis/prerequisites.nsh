; CodeConvoy: check actual system DLL versions and PE architectures, never registry
; claims alone. A stale or missing registry entry cannot override file evidence.
; Runs inside the existing x86 NSIS host, before the Rust utility plugin is used.
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
  !define CODECONVOY_CRT_X86 "$WINDIR\SysWOW64"
  !define CODECONVOY_CRT_X64 "$WINDIR\Sysnative"
!endif

!macro CrtReadWord HANDLE VALUE TEMP
  FileReadByte ${HANDLE} ${VALUE}
  FileReadByte ${HANDLE} ${TEMP}
  IntOp ${TEMP} ${TEMP} << 8
  IntOp ${VALUE} ${VALUE} | ${TEMP}
!macroend

!macro CodeConvoyRuntimeFunctions PREFIX CHECK_APP
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
  ${If} $CrtFileOK == 1
  ${AndIf} $CrtMachine == 0x14c
    ; The x86 host can also verify actual loading of the helper's dependencies.
    ; System-only dependency search prevents PATH/current-directory fallbacks.
    System::Call 'kernel32::SetErrorMode(i 0x8003)i.r3'
    System::Call 'kernel32::LoadLibraryExW(w "$CrtFile", p 0, i 0x800)p.r4'
    System::Call 'kernel32::SetErrorMode(i r3)'
    ${If} $4 == 0
      StrCpy $CrtFileOK 0
    ${Else}
      System::Call 'kernel32::FreeLibrary(p r4)'
    ${EndIf}
  ${EndIf}
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
  StrCpy $CrtDirectory "${CODECONVOY_CRT_X86}"
  StrCpy $CrtMachine 0x14c
  StrCpy $CrtArchitectureOK 1
@CHECKS_X86@
  ${If} $CrtArchitectureOK != 1
    StrCpy $CrtMissing "x86 runtime (required by installer process checks)"
  ${EndIf}
  !if ${CHECK_APP} == 1
    StrCpy $CrtDirectory "${CODECONVOY_CRT_X64}"
    StrCpy $CrtMachine 0x8664
    StrCpy $CrtArchitectureOK 1
@CHECKS_X64@
    ${If} $CrtArchitectureOK != 1
      ${If} $CrtMissing != ""
        StrCpy $CrtMissing "$CrtMissing$\r$\n"
      ${EndIf}
      StrCpy $CrtMissing "x64 runtime (required by CodeConvoy)$\r$\n$CrtMissing"
    ${EndIf}
  !endif
FunctionEnd

Function ${PREFIX}RequireWindowsRuntime
  Call ${PREFIX}CheckWindowsRuntime
  ${If} $CrtMissing != ""
    StrCpy $CrtMissing "Missing, outdated or unusable Microsoft runtime:$\r$\n$CrtMissing$\r$\n$\r$\nInstall or repair Microsoft Visual C++ v14 Redistributable @VC_VERSION@ or newer for the architecture(s) above, then retry. Keep Windows updated for the Universal CRT.$\r$\n$\r$\n@DOWNLOAD_PAGE@$\r$\n$\r$\nCodeConvoy does not download or install these prerequisites."
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

!insertmacro CodeConvoyRuntimeFunctions "" 1
; Removal needs only the x86 helper. Missing x64 application runtimes must not
; prevent removal, but missing x86 runtimes must never bypass process checks.
!insertmacro CodeConvoyRuntimeFunctions "un." 0
