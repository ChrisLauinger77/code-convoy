$ErrorActionPreference = 'Stop'
$version = python packaging/release.py version
if ($LASTEXITCODE -ne 0) { throw 'Cannot read Cargo version' }
$setup = (Resolve-Path "dist/CodeConvoy-$version-windows-x86_64-setup.exe").Path
$portable = Join-Path $env:RUNNER_TEMP 'CodeConvoy-portable'
$installed = Join-Path $env:LOCALAPPDATA 'CodeConvoy'
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\CodeConvoy'
if (Test-Path $installed) { throw 'Refusing to overwrite a previous installation during smoke test' }
Expand-Archive "dist/CodeConvoy-$version-windows-x86_64.zip" $portable
$info = (Get-Item "$portable/CodeConvoy.exe").VersionInfo
if ($info.ProductName -ne 'CodeConvoy' -or $info.ProductVersion -ne $version) {
    throw "Unexpected executable identity: $($info.ProductName) $($info.ProductVersion)"
}
try {
    $installer = Start-Process $setup -ArgumentList '/S' -Wait -PassThru
    if ($installer.ExitCode -ne 0) { throw "Installer failed: $($installer.ExitCode)" }
    if (!(Test-Path "$installed/CodeConvoy.exe") -or !(Test-Path "$installed/uninstall.exe")) {
        throw 'Installer did not create the executable and uninstaller'
    }
    $entry = Get-ItemProperty $uninstallKey
    if ($entry.DisplayName -ne 'CodeConvoy' -or $entry.DisplayVersion -ne $version) {
        throw 'Incorrect per-user uninstall registration'
    }
    $shortcuts = Get-ChildItem ([Environment]::GetFolderPath('StartMenu')) -Recurse -Filter 'CodeConvoy.lnk'
    if (!$shortcuts) { throw 'Start Menu shortcut is missing' }
    if ((Get-FileHash "$installed/CodeConvoy.exe").Hash -ne (Get-FileHash "$portable/CodeConvoy.exe").Hash) {
        throw 'Installed and portable executables differ'
    }
    # User data shares this per-user parent directory; uninstall must leave it.
    New-Item -ItemType Directory -Path "$installed/data" | Out-Null
    Set-Content "$installed/data/preservation-fixture.txt" 'keep user state'
} finally {
    if (Test-Path "$installed/uninstall.exe") {
        # NSIS _?= keeps the uninstaller in place so -Wait observes completion.
        $uninstaller = Start-Process "$installed/uninstall.exe" -ArgumentList "/S _?=$installed" -Wait -PassThru
        if ($uninstaller.ExitCode -ne 0) { throw "Uninstall failed: $($uninstaller.ExitCode)" }
    }
}
if (Test-Path "$installed/CodeConvoy.exe") { throw 'Uninstall left the application executable' }
if (Test-Path $uninstallKey) { throw 'Uninstall left its registration' }
if ((Get-Content "$installed/data/preservation-fixture.txt") -ne 'keep user state') {
    throw 'Uninstall did not preserve the user-data directory'
}
Write-Output 'Windows metadata, per-user installation, Start Menu, portable identity, and uninstall checks passed.'
Write-Output 'Interactive graphics, picker, and desktop cancellation still require a Windows desktop check.'
