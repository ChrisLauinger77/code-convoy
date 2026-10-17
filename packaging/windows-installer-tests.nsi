; Test-only executable. Never included in an installer or portable package.
Unicode true
RequestExecutionLevel user
SilentInstall silent
Name "CodeConvoy native installer tests"
OutFile "${OUTPUT}"
!include FileFunc.nsh
!include "windows-installer.nsh"

!macro AssertVersion NEW OLD EXPECTED
  Push "stack-marker"
  Push "${OLD}"
  Push "${NEW}"
  Call CompareReleaseVersions
  Pop $0
  StrCmp $0 "${EXPECTED}" +3
    SetErrorLevel 1603
    Quit
  Pop $0
  StrCmp $0 "stack-marker" +3
    SetErrorLevel 1603
    Quit
!macroend

Section
  ${GetOptions} $CMDLINE "/CASE=" $R0
  StrCmp $R0 "versions" versions
  StrCmp $R0 "identity" identity
  StrCmp $R0 "bad-identity" bad_identity
  StrCmp $R0 "timeout" timeout
  StrCmp $R0 "detect" detect
  StrCmp $R0 "stop" stop
  SetErrorLevel 1603
  Quit
  versions:
    !insertmacro AssertVersion "1.10.0" "1.9.0" 1
    !insertmacro AssertVersion "1.9.0" "1.10.0" -1
    !insertmacro AssertVersion "1.1.0" "1.0.0" 1
    !insertmacro AssertVersion "1.0.0" "1.0.0" 0
    !insertmacro AssertVersion "65535.0.0" "10.0.0" 1
    !insertmacro AssertVersion "0.0.0" "0.0.0" 0
    !insertmacro AssertVersion "1.0.0" "" 1603
    !insertmacro AssertVersion "1.0.0" "1.0" 1603
    !insertmacro AssertVersion "1.0.0" "1.0.0.0" 1603
    !insertmacro AssertVersion "1.0.0" "1..0" 1603
    !insertmacro AssertVersion "1.0.0" "1.0.0-beta" 1603
    !insertmacro AssertVersion "1.0.0" "1.0.0+build" 1603
    !insertmacro AssertVersion "1.0.0" "01.0.0" 1603
    !insertmacro AssertVersion "1.0.0" "1.0.65536" 1603
    !insertmacro AssertVersion "1.0.0" "1.0.9999999999" 1603
    !insertmacro AssertVersion "1.0.0" "1.0.-1" 1603
    !insertmacro AssertVersion "1.0.0" "1.0. 1" 1603
    !insertmacro AssertVersion "invalid" "1.0.0" 1603
    SetErrorLevel 0
    Quit
  identity:
    ClearErrors
    CreateShortcut "$EXEDIR\Shortcut ü 空.lnk" "$EXEDIR\CodeConvoyNsisFixture.exe" "/d /q /k"
    IfErrors bad_identity
    Push "$EXEDIR\Shortcut ü 空.lnk"
    Goto set_identity
  bad_identity:
    Push "$EXEDIR\missing\Shortcut.lnk"
  set_identity:
    Push "io.github.chrislauinger77.code-convoy"
    Call SetShortcutIdentity
    Pop $0
    SetErrorLevel $0
    Quit
  timeout:
    ${GetOptions} $CMDLINE "/PID=" $0
    System::Call 'kernel32::OpenProcess(i 0x100000, i 0, i r0)p.r1'
    StrCmp $1 0 wait_failure
    Push $1
    Push 25
    Call WaitForProcessExit
    Pop $0
    System::Call 'kernel32::CloseHandle(p r1)'
    Goto wait_done
  wait_failure:
    StrCpy $0 1603
  wait_done:
    SetErrorLevel $0
    Quit
  detect:
    Push "cOdEcOnVoYnSiSfIxTuRe.ExE"
    Push 0
    Goto process
  stop:
    Push "cOdEcOnVoYnSiSfIxTuRe.ExE"
    Push 1
  process:
    Call CheckAppProcesses
    Pop $0
    SetErrorLevel $0
SectionEnd
