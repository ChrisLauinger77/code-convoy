param([Parameter(Mandatory=$true)][string]$Work)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Use a disposable Actions Windows account.' }
$Work = (Resolve-Path $Work).Path
$policy = Get-Content 'packaging/windows-runtime.json' -Raw | ConvertFrom-Json
$results = [System.Collections.Generic.List[object]]::new()
function Restore-Fixture {
    foreach ($arch in @('x86', 'x64')) {
        $directory = Join-Path $Work "runtime-$arch"
        New-Item -ItemType Directory -Force $directory | Out-Null
        $system = if ($arch -eq 'x64') { 'System32' } else { 'SysWOW64' }
        foreach ($file in $policy.runtime_files.$arch) {
            Copy-Item (Join-Path $env:SystemRoot "$system/$file") (Join-Path $directory $file) -Force
        }
    }
    Remove-Item "$Work/*-helper.marker" -ErrorAction SilentlyContinue
}
function Check-Case([string]$name, [int]$expected, [string]$exe = 'prerequisite-tests.exe', [switch]$Uninstall) {
    $arguments = if ($Uninstall) { "/S _?=$Work" } else { '/S' }
    $process = Start-Process (Join-Path $Work $exe) -ArgumentList $arguments -PassThru
    if (!$process.WaitForExit(30000)) { $process.Kill(); throw "$name timed out" }
    $marker = if ($Uninstall) { 'uninstaller-helper.marker' } else { 'installer-helper.marker' }
    $exists = Test-Path "$Work/$marker"
    if ($process.ExitCode -ne $expected -or $exists -ne ($expected -eq 0)) {
        throw "$name failed: exit=$($process.ExitCode), expected=$expected, marker=$exists"
    }
    $results.Add(@{case=$name; exit_code=$process.ExitCode; helper_executed=($expected -eq 0)})
}
try {
    Restore-Fixture
    Check-Case 'both architectures installed' 0
    Restore-Fixture
    Remove-Item "$Work/runtime-x64/vcruntime140.dll"
    Check-Case 'x64 missing' 1603
    Restore-Fixture
    Remove-Item "$Work/runtime-x86/vcruntime140.dll"
    Check-Case 'x86 missing' 1603
    Restore-Fixture
    Remove-Item "$Work/runtime-x64/vcruntime140.dll", "$Work/runtime-x86/vcruntime140.dll"
    Check-Case 'both missing' 1603
    Restore-Fixture
    Remove-Item "$Work/runtime-x86/ucrtbase.dll"
    Check-Case 'x86 UCRT missing' 1603
    Restore-Fixture
    Remove-Item "$Work/runtime-x64/ucrtbase.dll"
    Check-Case 'x64 UCRT missing' 1603
    Restore-Fixture
    Copy-Item "$Work/runtime-x64/vcruntime140.dll" "$Work/runtime-x86/vcruntime140.dll" -Force
    Check-Case 'wrong DLL architecture' 1603
    Restore-Fixture
    Check-Case 'newer compatible files' 0 'prerequisite-newer.exe'
    Restore-Fixture
    Check-Case 'older than required version' 1603 'prerequisite-older.exe'
    Restore-Fixture
    Check-Case 'runtime removed after initial check' 1603 'prerequisite-changed.exe'
    Restore-Fixture
    Check-Case 'create fixture uninstaller' 0
    Restore-Fixture
    Remove-Item "$Work/runtime-x86/vcruntime140.dll"
    Check-Case 'uninstall x86 missing' 1603 'prerequisite-uninstall.exe' -Uninstall
    Restore-Fixture
    Remove-Item "$Work/runtime-x64/vcruntime140.dll"
    Check-Case 'uninstall does not need x64 application runtime' 0 'prerequisite-uninstall.exe' -Uninstall
    Restore-Fixture
    # Registry claims are deliberately not trusted. Exercise stale claims in
    # both views of a private test key; never edit real runtime registration.
    $keyName = "Software\CodeConvoyRuntimeFixture\$([guid]::NewGuid())"
    foreach ($view in @([Microsoft.Win32.RegistryView]::Registry32, [Microsoft.Win32.RegistryView]::Registry64)) {
        $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser, $view)
        try {
            $key = $base.CreateSubKey($keyName)
            try { $key.SetValue('Installed', 1); $key.SetValue('Version', 'v14.99.65535.0') } finally { $key.Dispose() }
            Remove-Item "$Work/runtime-x86/vcruntime140.dll" -ErrorAction SilentlyContinue
            Check-Case "stale registry claim $view" 1603
        } finally { $base.DeleteSubKeyTree($keyName, $false); $base.Dispose() }
    }
    $results | ConvertTo-Json -Depth 4 | Set-Content "$Work/results.json"
    Write-Output "Native prerequisite cases passed: $($results.Count). No system DLL or real runtime registration was changed."
} finally {
    # Microsoft files exist only transiently in this private test directory.
    foreach ($arch in @('x86','x64')) { Remove-Item "$Work/runtime-$arch" -Recurse -Force -ErrorAction SilentlyContinue }
}
