param([Parameter(Mandatory)][string]$MakeNSIS)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Use a disposable Windows CI account for native installer tests.' }
$work = Join-Path $env:RUNNER_TEMP "CodeConvoy NSIS ü 空-$([guid]::NewGuid())"
New-Item $work -ItemType Directory | Out-Null
$fixture = Join-Path $work 'native-tests.exe'
& $MakeNSIS /INPUTCHARSET UTF8 "/DOUTPUT=$fixture" "$PSScriptRoot/windows-installer-tests.nsi"
if ($LASTEXITCODE -ne 0) { throw 'Cannot compile native installer fixture' }

# Reuse a Windows executable as the x64 process fixture; no C++/Rust build.
# Hold cmd's input pipe open so it remains alive without spawning children.
Copy-Item "$env:WINDIR/System32/cmd.exe" "$work/CodeConvoyNsisFixture.exe"
Copy-Item "$env:WINDIR/System32/cmd.exe" "$work/UnrelatedFixture.exe"
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class NativeInstallerTest {
    [StructLayout(LayoutKind.Sequential)]
    private struct Privileges { public uint count, low; public int high; public uint attributes; }
    [DllImport("kernel32.dll")]
    private static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll")]
    private static extern bool CloseHandle(IntPtr handle);
    [DllImport("advapi32.dll", SetLastError=true)]
    private static extern bool OpenProcessToken(IntPtr process, uint access, out IntPtr token);
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    private static extern bool LookupPrivilegeValueW(string system, string name, out long luid);
    [DllImport("advapi32.dll", SetLastError=true)]
    private static extern bool AdjustTokenPrivileges(IntPtr token, bool all, ref Privileges privileges, uint size, IntPtr previous, IntPtr returned);
    public static void RemoveDebugPrivilege() {
        IntPtr token;
        long luid;
        if (!OpenProcessToken(GetCurrentProcess(), 0x20, out token)) throw new System.ComponentModel.Win32Exception();
        try {
            if (!LookupPrivilegeValueW(null, "SeDebugPrivilege", out luid)) throw new System.ComponentModel.Win32Exception();
            var privileges = new Privileges { count=1, low=(uint)luid, high=(int)(luid >> 32), attributes=4 };
            if (!AdjustTokenPrivileges(token, false, ref privileges, 0, IntPtr.Zero, IntPtr.Zero)) throw new System.ComponentModel.Win32Exception();
        } finally { CloseHandle(token); }
    }
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    public static extern bool ConvertStringSecurityDescriptorToSecurityDescriptorW(string s, uint rev, out IntPtr sd, out uint size);
    [DllImport("advapi32.dll", SetLastError=true)]
    public static extern bool SetKernelObjectSecurity(IntPtr handle, uint info, IntPtr sd);
    [DllImport("kernel32.dll")]
    public static extern IntPtr LocalFree(IntPtr memory);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    public static extern bool GetBinaryTypeW(string path, out uint type);
}
'@
$type = [uint32]0
if (![NativeInstallerTest]::GetBinaryTypeW("$work/CodeConvoyNsisFixture.exe", [ref]$type) -or $type -ne 6) {
    throw 'Process fixture must be native x64'
}
$results = [System.Collections.Generic.List[object]]::new()
$processes = [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
function Start-Fixture([string]$name = 'CodeConvoyNsisFixture.exe') {
    $info = [System.Diagnostics.ProcessStartInfo]::new((Join-Path $work $name), '/d /q /k')
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = [System.Diagnostics.Process]::Start($info)
    $null = $process.Handle # retain a cleanup handle before restricting access
    $processes.Add($process)
    if ($process.WaitForExit(200)) { throw 'Process fixture exited prematurely' }
    return $process
}
function Restrict-Fixture($process, [string]$sddl) {
    $sd = [IntPtr]::Zero
    $size = [uint32]0
    if (![NativeInstallerTest]::ConvertStringSecurityDescriptorToSecurityDescriptorW($sddl, 1, [ref]$sd, [ref]$size)) {
        throw 'Cannot create test-only process security descriptor'
    }
    try {
        if (![NativeInstallerTest]::SetKernelObjectSecurity($process.Handle, 4, $sd)) {
            throw 'Cannot set test-only process access restrictions'
        }
    } finally { $null = [NativeInstallerTest]::LocalFree($sd) }
}
function Assert-Case([string]$case, [int]$expected, [string]$extra = '') {
    $run = Start-Process $fixture -ArgumentList "/S /CASE=$case $extra" -PassThru
    if (!$run.WaitForExit(30000)) { $run.Kill(); throw "Native case hung: $case" }
    if ($run.ExitCode -ne $expected) { throw "$case returned $($run.ExitCode), expected $expected" }
    $results.Add(@{case=$case; expected=$expected; actual=$run.ExitCode; extra=$extra})
    Write-Output "Passed: $case (exit $expected)"
}
try {
    Assert-Case versions 0
    Assert-Case detect 0
    $unrelated = Start-Fixture 'UnrelatedFixture.exe'
    $first = Start-Fixture
    $second = Start-Fixture
    Assert-Case detect 1
    Assert-Case timeout 1603 "/PID=$($first.Id)"
    if ($first.HasExited) { throw 'Timeout check unexpectedly terminated its process' }
    # PowerShell/.NET on an administrative hosted runner can enable debug
    # privilege. Remove it from this disposable test parent's token so children
    # exercise ordinary per-user access checks. Production never enables it.
    [NativeInstallerTest]::RemoveDebugPrivilege()
    # Deny query access; detection must fail instead of claiming absence.
    Restrict-Fixture $first 'D:(D;;0x1000;;;WD)(A;;GA;;;WD)'
    try { Assert-Case detect 1603 } finally { Restrict-Fixture $first 'D:(A;;GA;;;WD)' }
    # Deny termination while leaving detection possible. No debug privilege.
    Restrict-Fixture $first 'D:(D;;0x1;;;WD)(A;;GA;;;WD)'
    try {
        Assert-Case detect 1
        Assert-Case stop 1603
        if ($first.HasExited) { throw 'Denied process was terminated' }
    } finally { Restrict-Fixture $first 'D:(A;;GA;;;WD)' }
    # Ensure multiple live matches regardless of snapshot order in denied test.
    if ($second.HasExited) { $second = Start-Fixture }
    Assert-Case stop 0
    if (!$first.WaitForExit(1000) -or !$second.WaitForExit(1000)) { throw 'Termination not confirmed' }
    if ($unrelated.HasExited) { throw 'An unrelated process was terminated' }
    Assert-Case detect 0
    Assert-Case stop 0
    Assert-Case identity 0
    $shell = New-Object -ComObject Shell.Application
    $item = $shell.Namespace($work).ParseName('Shortcut ü 空.lnk')
    if ($item.ExtendedProperty('System.AppUserModel.ID') -ne 'io.github.chrislauinger77.code-convoy') {
        throw 'Shortcut identity readback failed'
    }
    if ($item.ExtendedProperty('System.Link.TargetParsingPath') -ne "$work\CodeConvoyNsisFixture.exe") {
        throw 'Unicode shortcut target changed'
    }
    Assert-Case bad-identity 1603
    # Run the detector with the very name it scans for: self must be excluded.
    Copy-Item $fixture "$work/CodeConvoyNsisFixture.exe" -Force
    $self = Start-Process "$work/CodeConvoyNsisFixture.exe" -ArgumentList '/S /CASE=detect' -Wait -PassThru
    if ($self.ExitCode -ne 0) { throw 'Installer-process exclusion failed' }
    $results.Add(@{case='self-exclusion'; actual=$self.ExitCode})
    $results | ConvertTo-Json | Write-Output
    Write-Output 'All focused native Windows installer tests passed.'
} finally {
    foreach ($process in $processes) {
        if (!$process.HasExited) {
            $process.StandardInput.WriteLine('exit')
            if (!$process.WaitForExit(3000)) { $process.Kill(); $process.WaitForExit() }
        }
        $process.Dispose()
    }
    # Only the newly created GUID-named fixture directory is removed.
    Remove-Item -LiteralPath $work -Recurse -Force
}
