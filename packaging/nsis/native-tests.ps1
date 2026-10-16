$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') {
    throw 'Run native tests in a disposable GitHub Actions Windows account.'
}
if (Get-Process CodeConvoyNsisFixture -ErrorAction SilentlyContinue) {
    throw 'Refusing to interfere with an existing process fixture'
}
$work = (Resolve-Path 'target/packaging/nsis-tests').Path
$one = $null
$two = $null
try {
    $one = Start-Process "$work/CodeConvoyNsisFixture.exe" -PassThru
    $two = Start-Process "$work/CodeConvoyNsisFixture.exe" -PassThru
    Start-Sleep -Milliseconds 500
    if ($one.HasExited -or $two.HasExited) { throw 'Process fixtures did not start' }
    $test = Start-Process "$work/helper-tests.exe" -Wait -PassThru
    if ($test.ExitCode -ne 0) { throw "NSIS helper ABI test failed: $($test.ExitCode)" }
    if (!$one.WaitForExit(5000) -or !$two.WaitForExit(5000)) { throw 'KillProcess did not terminate both fixtures' }
    $shell = New-Object -ComObject Shell.Application
    $item = $shell.Namespace($work).ParseName('Shortcut ü 空.lnk')
    if ($item.ExtendedProperty('System.AppUserModel.ID') -ne 'io.github.chrislauinger77.code-convoy') {
        throw 'Shortcut AppUserModelID was not saved by the native helper'
    }
    Write-Output 'Native NSIS ABI: version ordering, invalid versions, stack balance, Unicode shortcut identity, find/kill/missing/multiple processes passed.'
} finally {
    foreach ($process in @($one, $two)) {
        if ($null -ne $process -and !$process.HasExited) { $process.Kill(); $process.WaitForExit() }
    }
}
