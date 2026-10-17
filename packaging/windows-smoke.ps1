param([switch]$UnicodePath, [switch]$ReleasedUpgrade)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Run installer tests in a disposable GitHub Actions Windows account.' }
if (-not ('CodeConvoySmoke.Paths' -as [type])) {
    Add-Type @'
using System.Runtime.InteropServices;
using System.Text;
namespace CodeConvoySmoke {
    public static class Paths {
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern uint GetLongPathName(string path, StringBuilder result, uint size);
    }
}
'@
}
function Get-LongPath([string]$path) {
    $result = [System.Text.StringBuilder]::new(32768)
    $size = [CodeConvoySmoke.Paths]::GetLongPathName($path, $result, 32768)
    if ($size -eq 0 -or $size -ge 32768) { throw "Cannot resolve shortcut path: $path" }
    return $result.ToString()
}
$version = python packaging/release.py version
if ($LASTEXITCODE -ne 0) { throw 'Cannot read Cargo version' }
$setup = (Resolve-Path "dist/CodeConvoy-$version-windows-x86_64-setup.exe").Path
$portable = Join-Path $env:RUNNER_TEMP "CodeConvoy-portable-$([guid]::NewGuid())"
$installed = if ($UnicodePath) { Join-Path $env:RUNNER_TEMP 'CodeConvoy 安装 with spaces' } elseif ($ReleasedUpgrade) { Join-Path $env:RUNNER_TEMP 'CodeConvoy legacy migration' } else { Join-Path $env:LOCALAPPDATA 'CodeConvoy' }
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\CodeConvoy'
if (Test-Path $installed) { throw 'Refusing to overwrite a previous installation during smoke test' }
if (Test-Path $uninstallKey) { throw 'Refusing to overwrite existing application registration' }
if (Get-Process CodeConvoy -ErrorAction SilentlyContinue) { throw 'Refusing to stop an existing CodeConvoy process' }
$desktopShortcut = Join-Path ([Environment]::GetFolderPath('Desktop')) 'CodeConvoy.lnk'
$existingShortcuts = Get-ChildItem ([Environment]::GetFolderPath('StartMenu')) -Recurse -Filter 'CodeConvoy.lnk'
if ((Test-Path $desktopShortcut) -or $existingShortcuts) { throw 'Refusing to overwrite existing CodeConvoy shortcuts' }
Expand-Archive "dist/CodeConvoy-$version-windows-x86_64.zip" $portable
if ((Get-FileHash "$portable/LICENSE.txt").Hash -ne (Get-FileHash 'LICENSE').Hash) {
    throw 'Portable archive license is missing or incorrect'
}
python packaging/licenses.py verify $portable
if ($LASTEXITCODE -ne 0) { throw 'Portable third-party materials failed verification' }
$info = (Get-Item "$portable/CodeConvoy.exe").VersionInfo
if ($info.ProductName -ne 'CodeConvoy' -or $info.ProductVersion -ne $version) {
    throw "Unexpected executable identity: $($info.ProductName) $($info.ProductVersion)"
}
$running = $null
try {
    $running = Start-Process "$portable/CodeConvoy.exe" -PassThru
    Start-Sleep -Seconds 3
    if ($running.HasExited) { throw 'Portable application could not start with the installed runtime' }
    if (!$running.CloseMainWindow() -or !$running.WaitForExit(15000)) {
        throw 'Portable application did not close normally'
    }
    $running = $null
    if ($ReleasedUpgrade) {
        . "$PSScriptRoot/windows-upgrade.ps1"
        Test-ReleasedUpgrade $installed $setup -Overwrite:$UnicodePath
    }
    if (!$ReleasedUpgrade) {
        $minimal = Start-Process $setup -ArgumentList "/S /NS /R /D=$installed" -PassThru
        if (!$minimal.WaitForExit(90000)) { $minimal.Kill(); throw '/NS /R installer timed out' }
        if ($minimal.ExitCode -ne 0) { throw 'Fresh /NS /R installation failed' }
        if ((Test-Path $desktopShortcut) -or (Get-ChildItem ([Environment]::GetFolderPath('StartMenu')) -Recurse -Filter 'CodeConvoy.lnk')) {
            throw '/NS created a shortcut'
        }
        Start-Sleep -Seconds 3
        $started = @(Get-Process CodeConvoy -ErrorAction SilentlyContinue)
        if ($started.Count -ne 1) { throw '/R did not start CodeConvoy' }
        $running = $started[0]
        if (!$running.CloseMainWindow() -or !$running.WaitForExit(15000)) { throw '/R application did not close normally' }
        $running = $null
    }
    # NSIS requires the unquoted /D= argument last, including embedded spaces.
    $installer = Start-Process $setup -ArgumentList "/S /D=$installed" -Wait -PassThru
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
    $shell = New-Object -ComObject Shell.Application
    foreach ($shortcut in @($shortcuts.FullName) + @($desktopShortcut)) {
        if (!(Test-Path $shortcut)) { throw "Missing shortcut: $shortcut" }
        $folder = $shell.Namespace((Split-Path $shortcut))
        $item = $folder.ParseName((Split-Path $shortcut -Leaf))
        if ($item.ExtendedProperty('System.AppUserModel.ID') -ne 'io.github.chrislauinger77.code-convoy') {
            throw "Incorrect shortcut AppUserModelID: $shortcut"
        }
        # Read the Shell's Unicode target property; WScript.Shell's legacy
        # TargetPath reader replaces characters outside the active code page.
        # https://learn.microsoft.com/en-us/windows/win32/properties/props-system-link-targetparsingpath
        $actualTarget = $item.ExtendedProperty('System.Link.TargetParsingPath')
        if ((Get-LongPath $actualTarget) -ne (Get-LongPath "$installed\CodeConvoy.exe")) {
            throw "Incorrect shortcut target: '$actualTarget'; expected '$installed\CodeConvoy.exe'"
        }
    }
    if ((Get-FileHash "$installed/CodeConvoy.exe").Hash -ne (Get-FileHash "$portable/CodeConvoy.exe").Hash) {
        throw 'Installed and portable executables differ'
    }
    if ((Get-FileHash "$installed/LICENSE.txt").Hash -ne (Get-FileHash 'LICENSE').Hash) {
        throw 'Installed license is missing or incorrect'
    }
    python packaging/licenses.py verify $installed
    if ($LASTEXITCODE -ne 0) { throw 'Installed third-party materials failed verification' }
    $nativeFiles = @(Get-ChildItem $installed -Recurse -File | Where-Object { $_.Extension -in '.exe', '.dll' } | ForEach-Object { $_.Name } | Sort-Object)
    if (($nativeFiles -join ',') -ne 'CodeConvoy.exe,uninstall.exe') {
        throw "Unreviewed installed native components: $($nativeFiles -join ',')"
    }
    # Malformed registration must fail in silent mode before touching files.
    Set-ItemProperty $uninstallKey DisplayVersion '1.0.0+unsupported'
    try {
        $invalid = Start-Process $setup -ArgumentList "/S /D=$installed" -Wait -PassThru
        if ($invalid.ExitCode -ne 1603) { throw 'Malformed installed version did not fail with 1603' }
        if ((Get-FileHash "$installed/CodeConvoy.exe").Hash -ne (Get-FileHash "$portable/CodeConvoy.exe").Hash) {
            throw 'Rejected maintenance changed the application'
        }
    } finally { Set-ItemProperty $uninstallKey DisplayVersion $version }
    # User data shares this per-user parent directory; uninstall must leave it.
    # Portable startup may already have created this application data folder.
    New-Item -ItemType Directory -Force -Path "$installed/data" | Out-Null
    Set-Content "$installed/data/preservation-fixture.txt" 'keep user state'
    Set-Content "$installed/unrelated-user-file.txt" 'keep unrelated file'
    # The released-upgrade case covers real version changes. Both unattended
    # modes must also safely reinstall an existing running application.
    foreach ($mode in @('/S', '/P')) {
        $running = Start-Process "$installed/CodeConvoy.exe" -PassThru
        Start-Sleep -Seconds 3
        if ($running.HasExited) { throw 'CodeConvoy did not remain running for the installer process test' }
        $maintenance = Start-Process $setup -ArgumentList "$mode /D=$installed" -PassThru
        if (!$maintenance.WaitForExit(90000)) { $maintenance.Kill(); throw 'Installer maintenance timed out' }
        if ($maintenance.ExitCode -ne 0 -or !$running.WaitForExit(5000)) { throw 'Running application handling failed' }
        if ((Get-ItemProperty $uninstallKey).DisplayVersion -ne $version) { throw 'Upgrade/downgrade registration failed' }
        if ((Get-Content "$installed/data/preservation-fixture.txt") -ne 'keep user state') { throw 'Maintenance changed user data' }
        if ((Get-FileHash "$installed/CodeConvoy.exe").Hash -ne (Get-FileHash "$portable/CodeConvoy.exe").Hash) { throw 'Maintenance changed application identity' }
    }
    $running = Start-Process "$installed/CodeConvoy.exe" -PassThru
    Start-Sleep -Seconds 3
    if ($running.HasExited) { throw 'Application did not remain running for uninstall' }
} finally {
    if (Test-Path "$installed/uninstall.exe") {
        # NSIS _?= keeps the uninstaller in place so -Wait observes completion.
        $uninstaller = Start-Process "$installed/uninstall.exe" -ArgumentList "/S _?=$installed" -Wait -PassThru
        if ($uninstaller.ExitCode -ne 0) { throw "Uninstall failed: $($uninstaller.ExitCode)" }
        if ($null -ne $running -and !$running.WaitForExit(5000)) { throw 'Uninstall did not stop the running application' }
    }
}
if (Test-Path "$installed/CodeConvoy.exe") { throw 'Uninstall left the application executable' }
if (Test-Path "$installed/LICENSE.txt") { throw 'Uninstall left the packaged license' }
foreach ($name in @('THIRD-PARTY-NOTICES.txt', 'THIRD-PARTY-INVENTORY.json', 'THIRD-PARTY-SOURCES.zip', 'RUST-STDLIB-LICENSES.html')) {
    if (Test-Path "$installed/$name") { throw "Uninstall left third-party material: $name" }
}
if (Test-Path $uninstallKey) { throw 'Uninstall left its registration' }
if ((Test-Path $desktopShortcut) -or (Get-ChildItem ([Environment]::GetFolderPath('StartMenu')) -Recurse -Filter 'CodeConvoy.lnk')) {
    throw 'Uninstall left application shortcuts'
}
if ((Get-Content "$installed/data/preservation-fixture.txt") -ne 'keep user state') {
    throw 'Uninstall did not preserve the user-data directory'
}
if ((Get-Content "$installed/unrelated-user-file.txt") -ne 'keep unrelated file') { throw 'Uninstall removed unrelated user content' }
Write-Output "Windows per-user installation, shortcut AppUserModelID/targets, upgrade/downgrade registration, running application handling, portable identity, uninstall and data preservation passed: $installed"
Write-Output 'Interactive graphics, picker, and desktop cancellation still require a Windows desktop check.'
