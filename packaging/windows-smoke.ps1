param([switch]$UnicodePath)
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
$installed = if ($UnicodePath) { Join-Path $env:RUNNER_TEMP 'CodeConvoy 安装 with spaces' } else { Join-Path $env:LOCALAPPDATA 'CodeConvoy' }
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
    $wscript = New-Object -ComObject WScript.Shell
    foreach ($shortcut in @($shortcuts.FullName) + @($desktopShortcut)) {
        if (!(Test-Path $shortcut)) { throw "Missing shortcut: $shortcut" }
        $folder = $shell.Namespace((Split-Path $shortcut))
        if ($folder.ParseName((Split-Path $shortcut -Leaf)).ExtendedProperty('System.AppUserModel.ID') -ne 'io.github.chrislauinger77.code-convoy') {
            throw "Incorrect shortcut AppUserModelID: $shortcut"
        }
        $actualTarget = $wscript.CreateShortcut($shortcut).TargetPath
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
    # User data shares this per-user parent directory; uninstall must leave it.
    New-Item -ItemType Directory -Path "$installed/data" | Out-Null
    Set-Content "$installed/data/preservation-fixture.txt" 'keep user state'
    # Exercise upgrade/downgrade selection with synthetic prior registration
    # versions. /P traverses PageReinstall and its SemverCompare call.
    foreach ($previous in @('0.9.9', '1.0.1')) {
        Set-ItemProperty $uninstallKey DisplayVersion $previous
        $running = Start-Process "$installed/CodeConvoy.exe" -PassThru
        Start-Sleep -Seconds 3
        if ($running.HasExited) { throw 'CodeConvoy did not remain running for the installer process test' }
        $maintenance = Start-Process $setup -ArgumentList "/P /D=$installed" -PassThru
        if (!$maintenance.WaitForExit(90000)) { $maintenance.Kill(); throw 'Installer maintenance timed out' }
        if ($maintenance.ExitCode -ne 0 -or !$running.WaitForExit(5000)) { throw 'Running application handling failed' }
        if ((Get-ItemProperty $uninstallKey).DisplayVersion -ne $version) { throw 'Upgrade/downgrade registration failed' }
        if ((Get-Content "$installed/data/preservation-fixture.txt") -ne 'keep user state') { throw 'Maintenance changed user data' }
        if ((Get-FileHash "$installed/CodeConvoy.exe").Hash -ne (Get-FileHash "$portable/CodeConvoy.exe").Hash) { throw 'Maintenance changed application identity' }
    }
} finally {
    if ($null -ne $running -and !$running.HasExited) { $running.Kill(); $running.WaitForExit() }
    if (Test-Path "$installed/uninstall.exe") {
        # NSIS _?= keeps the uninstaller in place so -Wait observes completion.
        $uninstaller = Start-Process "$installed/uninstall.exe" -ArgumentList "/S _?=$installed" -Wait -PassThru
        if ($uninstaller.ExitCode -ne 0) { throw "Uninstall failed: $($uninstaller.ExitCode)" }
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
Write-Output "Windows per-user installation, shortcut AppUserModelID/targets, upgrade/downgrade registration, running application handling, portable identity, uninstall and data preservation passed: $installed"
Write-Output 'Interactive graphics, picker, and desktop cancellation still require a Windows desktop check.'
