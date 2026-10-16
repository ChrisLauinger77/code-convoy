; Disposable native ABI tests; never included in the shipped installer.
Unicode true
RequestExecutionLevel user
SilentInstall silent
OutFile "@OUTPUT@"
!include LogicLib.nsh
!addplugindir /x86-unicode "@HELPERS@"

!macro Compare left right result
  Push "sentinel"
  codeconvoy_utils::SemverCompare "${left}" "${right}"
  Pop $0
  ${If} $0 != "${result}"
    SetErrorLevel 20
    Quit
  ${EndIf}
  Pop $0
  ${If} $0 != "sentinel"
    SetErrorLevel 21
    Quit
  ${EndIf}
!macroend

Section
  !insertmacro Compare "1.0.0" "0.9.9" "1"
  !insertmacro Compare "1.0.0" "1.0.1" "-1"
  !insertmacro Compare "1.0.0" "1.0.0" "0"
  !insertmacro Compare "1.0.0-alpha.2" "1.0.0-alpha.10" "-1"
  !insertmacro Compare "1.0.0" "invalid" "1"
  !insertmacro Compare "invalid" "1.0.0" "-1"
  !insertmacro Compare "invalid" "also-invalid" "0"
  ; semver 1.0.18 Version::Ord includes build metadata, as upstream does.
  !insertmacro Compare "1.0.0+aaa" "1.0.0+bbb" "-1"

  CreateShortcut "$EXEDIR\Shortcut ü 空.lnk" "$SYSDIR\notepad.exe"
  Push "sentinel"
  CodeConvoyAppId::Set "$EXEDIR\Shortcut ü 空.lnk" "io.github.chrislauinger77.code-convoy"
  Pop $0
  ${If} $0 != "0"
    SetErrorLevel 30
    Quit
  ${EndIf}
  Pop $0
  ${If} $0 != "sentinel"
    SetErrorLevel 31
    Quit
  ${EndIf}
  CodeConvoyAppId::Set "$EXEDIR\does-not-exist.lnk" "io.github.chrislauinger77.code-convoy"
  Pop $0
  ${If} $0 != "1"
    SetErrorLevel 32
    Quit
  ${EndIf}
  codeconvoy_utils::FindProcess "CodeConvoyNsisFixture.exe"
  Pop $0
  ${If} $0 != "0"
    SetErrorLevel 40
    Quit
  ${EndIf}
  codeconvoy_utils::KillProcess "CODECONVOYNSISFIXTURE.EXE"
  Pop $0
  ${If} $0 != "0"
    SetErrorLevel 41
    Quit
  ${EndIf}
  Sleep 500
  codeconvoy_utils::FindProcess "CodeConvoyNsisFixture.exe"
  Pop $0
  ${If} $0 != "1"
    SetErrorLevel 42
    Quit
  ${EndIf}
  codeconvoy_utils::KillProcess "CodeConvoyNsisFixture.exe"
  Pop $0
  ${If} $0 != "1"
    SetErrorLevel 43
    Quit
  ${EndIf}
  SetErrorLevel 0
SectionEnd
