; Native installer operations. Only NSIS's bundled System plugin is required.
!ifndef CODECONVOY_WINDOWS_INSTALLER
!define CODECONVOY_WINDOWS_INSTALLER
!include LogicLib.nsh
!include WordFunc.nsh
!include Win/COM.nsh
!include Win/Propkey.nsh

; Push version; returns 0 or 1603. Windows release components are canonical
; decimal integers in [0,65535]; validating also bounds WordFunc's arithmetic.
Function ValidateReleaseVersion
  System::Store S
  Pop $0
  StrCpy $1 0 ; character offset
  StrCpy $2 0 ; number of separators
  StrCpy $3 "" ; component
  StrCpy $9 1603
  next_character:
    StrCpy $4 $0 1 $1
    StrCmp $4 "" component
    StrCmp $4 "." component
    StrCmp $4 "0" digit
    StrCmp $4 "1" digit
    StrCmp $4 "2" digit
    StrCmp $4 "3" digit
    StrCmp $4 "4" digit
    StrCmp $4 "5" digit
    StrCmp $4 "6" digit
    StrCmp $4 "7" digit
    StrCmp $4 "8" digit
    StrCmp $4 "9" digit done
  digit:
    StrCpy $3 "$3$4"
    StrLen $5 $3
    IntCmp $5 5 advance advance done
  advance:
    IntOp $1 $1 + 1
    Goto next_character
  component:
    StrCmp $3 "" done
    StrLen $5 $3
    StrCpy $6 $3 1
    ${If} $5 > 1
    ${AndIf} $6 == "0"
      Goto done
    ${EndIf}
    IntCmp $3 65535 valid_component valid_component done
  valid_component:
    StrCmp $4 "" end_version
    IntOp $2 $2 + 1
    IntCmp $2 2 separator separator done
  separator:
    StrCpy $3 ""
    Goto advance
  end_version:
    ${If} $2 == 2
      StrCpy $9 0
    ${EndIf}
  done:
    Push $9
    System::Store L
FunctionEnd

; Push installed version, then new version. Returns 0 equal, 1 upgrade,
; -1 downgrade, or 1603 invalid. No prerelease/metadata coercion.
Function CompareReleaseVersions
  System::Store S
  Pop $0
  Pop $1
  Push $0
  Call ValidateReleaseVersion
  Pop $2
  StrCmp $2 0 0 done
  Push $1
  Call ValidateReleaseVersion
  Pop $2
  StrCmp $2 0 0 done
  ${VersionCompare} $0 $1 $2
  ${If} $2 == 2
    StrCpy $2 -1
  ${EndIf}
  done:
    Push $2
    System::Store L
FunctionEnd

!macro CodeConvoyProcessFunctions PREFIX
; Push process handle, then timeout in ms. Returns 0 only for a signaled
; process; timeout, WAIT_FAILED and unexpected results all return 1603.
; The caller owns the handle. Shared with the native timeout regression test.
Function ${PREFIX}WaitForProcessExit
  System::Store S
  Pop $1
  Pop $0
  System::Call 'kernel32::WaitForSingleObject(p r0, i r1)i.r2'
  ${If} $2 != 0
    StrCpy $2 1603
  ${EndIf}
  Push $2
  System::Store L
FunctionEnd

; Push executable basename, then 0 (detect) or 1 (terminate).
; Returns 0 absent/all stopped, 1 running (detect only), or 1603 uncertain.
; The name policy deliberately includes other installations of CodeConvoy.
; A process handle and its full image name are revalidated before termination
; so a recycled snapshot PID can never cause an unrelated process to be killed.
Function ${PREFIX}CheckAppProcesses
  System::Store S
  Pop $0 ; mode
  Pop $1 ; basename
  StrCpy $2 -1 ; snapshot
  StrCpy $3 0 ; PROCESSENTRY32W buffer
  StrCpy $4 0 ; full image name buffer
  StrCpy $5 0 ; process handle
  StrCpy $9 1603 ; result: fail closed
  StrCpy $R9 0 ; found
  StrCmp $1 "" cleanup
  ${If} $0 != 0
  ${AndIf} $0 != 1
    Goto cleanup
  ${EndIf}
  ; cargo-packager's NSIS host is Unicode x86, even for the x64 application.
  !if ${NSIS_PTR_SIZE} != 4
    !error "Recheck PROCESSENTRY32W layout for a non-x86 NSIS host"
  !endif
  System::Alloc 556
  Pop $3
  StrCmp $3 0 cleanup
  System::Alloc 65536
  Pop $4
  StrCmp $4 0 cleanup
  System::Call '*$3(i 556)'
  System::Call 'kernel32::GetCurrentProcessId()i.r6'
  System::Call 'kernel32::CreateToolhelp32Snapshot(i 2, i 0)p.r2'
  StrCmp $2 -1 cleanup
  System::Call 'kernel32::Process32FirstW(p r2, p r3)i.r7 ?e'
  Pop $8
  entry:
    ${If} $7 == 0
      StrCmp $8 18 complete cleanup ; ERROR_NO_MORE_FILES only
    ${EndIf}
    System::Call '*$3(i, i, i.r7, p, i, i, i, i, i, &w260.r8)'
    StrCmp $7 $6 next_entry ; installer-process exclusion
    StrCmp $8 $1 0 next_entry ; NSIS StrCmp is case-insensitive
    StrCpy $R0 0x101000 ; SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION
    ${If} $0 == 1
      IntOp $R0 $R0 | 1 ; PROCESS_TERMINATE
    ${EndIf}
    System::Call 'kernel32::OpenProcess(i R0, i 0, i r7)p.r5'
    StrCmp $5 0 cleanup ; includes access denied and disappeared/reused PID
    System::Call 'kernel32::WaitForSingleObject(p r5, i 0)i.r7'
    StrCmp $7 0 close_process ; already exited
    StrCmp $7 258 0 cleanup ; WAIT_TIMEOUT means still running
    StrCpy $R0 32768
    System::Call 'kernel32::QueryFullProcessImageNameW(p r5, i 0, p r4, *i R0)i.r7'
    StrCmp $7 0 cleanup
    System::Call 'shlwapi::PathFindFileNameW(p r4)p.r7'
    StrCmp $7 0 cleanup
    System::Call '*$7(&w260.r8)'
    StrCmp $8 $1 0 cleanup
    StrCpy $R9 1
    StrCmp $0 0 close_process
    System::Call 'kernel32::TerminateProcess(p r5, i 1)i.r7'
    StrCmp $7 0 cleanup
    Push $5
    Push 10000
    Call ${PREFIX}WaitForProcessExit
    Pop $7
    StrCmp $7 0 0 cleanup
  close_process:
    System::Call 'kernel32::CloseHandle(p r5)i.r7'
    StrCpy $5 0
    StrCmp $7 0 cleanup
  next_entry:
    System::Call 'kernel32::Process32NextW(p r2, p r3)i.r7 ?e'
    Pop $8
    Goto entry
  complete:
    StrCpy $9 $R9
    ${If} $0 == 1
      StrCpy $9 0
    ${EndIf}
  cleanup:
    ${If} $5 != 0
      System::Call 'kernel32::CloseHandle(p r5)i.r7'
      ${IfThen} $7 == 0 ${|} StrCpy $9 1603 ${|}
    ${EndIf}
    ${If} $2 != -1
      System::Call 'kernel32::CloseHandle(p r2)i.r7'
      ${IfThen} $7 == 0 ${|} StrCpy $9 1603 ${|}
    ${EndIf}
    System::Free $3
    System::Free $4
    ; Re-scan once after stopping all matches; a new/restarted process is an
    ; error, never an unbounded retry or permission to overwrite its files.
    ${If} $0 == 1
    ${AndIf} $9 == 0
      Push $1
      Push 0
      Call ${PREFIX}CheckAppProcesses
      Pop $9
      ${IfThen} $9 != 0 ${|} StrCpy $9 1603 ${|}
    ${EndIf}
    Push $9
    System::Store L
FunctionEnd
!macroend
!insertmacro CodeConvoyProcessFunctions ""
!insertmacro CodeConvoyProcessFunctions "un."

; Push shortcut path, then AppUserModelID. Returns 0 or 1603.
; CreateShortcut owns the target/arguments/icon. Only its identity is changed.
Function SetShortcutIdentity
  System::Store S
  Pop $1 ; ID
  Pop $0 ; path
  StrCpy $2 0 ; IShellLinkW
  StrCpy $3 0 ; IPersistFile
  StrCpy $4 0 ; IPropertyStore
  StrCpy $5 0 ; PROPERTYKEY
  StrCpy $6 0 ; PROPVARIANT
  StrCpy $7 0 ; CoTaskMem string before ownership transfer
  StrCpy $8 0 ; owns COM initialization
  StrCpy $9 1603
  StrCmp $0 "" cleanup
  StrCmp $1 "" cleanup
  System::Call 'ole32::CoInitializeEx(p 0, i 2)i.R0'
  ${If} $R0 == 0
  ${OrIf} $R0 == 1
    StrCpy $8 1 ; S_OK and S_FALSE each require CoUninitialize
  ${ElseIf} $R0 <> -2147417850 ; RPC_E_CHANGED_MODE: use existing apartment
    Goto cleanup
  ${EndIf}
  !insertmacro ComHlpr_CreateInProcInstance ${CLSID_ShellLink} ${IID_IShellLinkW} r2 ".R0"
  StrCmp $R0 0 0 cleanup
  StrCmp $2 0 cleanup
  ${IUnknown::QueryInterface} $2 '("${IID_IPersistFile}",.r3).R0'
  StrCmp $R0 0 0 cleanup
  StrCmp $3 0 cleanup
  ${IPersistFile::Load} $3 '("$0",${STGM_READWRITE}).R0'
  StrCmp $R0 0 0 cleanup
  ${IUnknown::QueryInterface} $2 '("${IID_IPropertyStore}",.r4).R0'
  StrCmp $R0 0 0 cleanup
  StrCmp $4 0 cleanup
  System::Call '*${SYSSTRUCT_PROPERTYKEY}(${PKEY_AppUserModel_ID})p.r5'
  StrCmp $5 0 cleanup
  System::Alloc ${SYSSIZEOF_PROPVARIANT}
  Pop $6
  StrCmp $6 0 cleanup
  System::Call '*$6${SYSSTRUCT_PROPVARIANT}(0,0,0)'
  ; SHStrDupW is exported; InitPropVariantFromString is only a C++ inline.
  System::Call 'shlwapi::SHStrDupW(w r1, *p.r7)i.R0'
  StrCmp $R0 0 0 cleanup
  StrCmp $7 0 cleanup
  ${V_SetPointer} $6 ${VT_LPWSTR} $7
  StrCpy $7 0 ; now owned by the PROPVARIANT
  ${IPropertyStore::SetValue} $4 '($5,$6).R0'
  StrCmp $R0 0 0 cleanup
  ${IPropertyStore::Commit} $4 '().R0'
  StrCmp $R0 0 0 cleanup
  ${IPersistFile::Save} $3 '("$0",1).R0'
  StrCmp $R0 0 0 cleanup ; S_FALSE is not a successful save
  StrCpy $9 0
  cleanup:
    ${If} $7 != 0
      System::Call 'ole32::CoTaskMemFree(p r7)'
    ${EndIf}
    ${If} $6 != 0
      System::Call 'ole32::PropVariantClear(p r6)i.R0'
      ${IfThen} $R0 != 0 ${|} StrCpy $9 1603 ${|}
      System::Free $6
    ${EndIf}
    System::Free $5
    !insertmacro ComHlpr_SafeRelease $4
    !insertmacro ComHlpr_SafeRelease $3
    !insertmacro ComHlpr_SafeRelease $2
    ${If} $8 == 1
      System::Call 'ole32::CoUninitialize()'
    ${EndIf}
    Push $9
    System::Store L
FunctionEnd
!endif
