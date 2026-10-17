# Sourced only by windows-smoke.ps1 in a disposable CI account. Exercise the
# released 1.0.0 installer and its real uninstaller, without changing Cargo.
function Test-ReleasedUpgrade([string]$installed, [string]$currentSetup, [switch]$Overwrite) {
    $work = Join-Path $env:RUNNER_TEMP "CodeConvoy upgrade-$([guid]::NewGuid())"
    New-Item $work -ItemType Directory | Out-Null
    $old = Join-Path $work 'released-1.0.0.exe'
    Invoke-WebRequest 'https://github.com/ChrisLauinger77/code-convoy/releases/download/v1.0.0/CodeConvoy-1.0.0-windows-x86_64-setup.exe' -OutFile $old
    if ((Get-FileHash $old).Hash -ne '048869715899d725a81c3ed66f1a741a36b0ff6014519e0d60d981a896b89cb6') {
        throw 'Released 1.0.0 installer differs from its published digest'
    }
    # Only the test installer metadata changes. Application Cargo files and the
    # inspected distributable remain unchanged; this fixture is never published.
    $config = Get-Content 'target/packaging/windows/packager.json' -Raw | ConvertFrom-Json
    $currentVersion = [version]$config.version
    $config.version = "$($currentVersion.Major).$($currentVersion.Minor + 1).0"
    $config.outDir = Join-Path $work 'next-version'
    $configFile = Join-Path $work 'packager.json'
    $config | ConvertTo-Json -Depth 20 | Set-Content $configFile
    & cargo packager --config (Get-Content $configFile -Raw) --formats nsis
    if ($LASTEXITCODE -ne 0) { throw 'Cannot compile next-version installer fixture' }
    $next = @(Get-ChildItem $config.outDir -Filter '*.exe')
    if ($next.Count -ne 1) { throw 'Expected one next-version installer fixture' }
    $install = Start-Process $old -ArgumentList "/S /D=$installed" -Wait -PassThru
    if ($install.ExitCode -ne 0) { throw "Released installer failed: $($install.ExitCode)" }
    $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\CodeConvoy'
    if ((Get-ItemProperty $key).DisplayVersion -ne '1.0.0') { throw 'Released installation not registered' }
    $legacyHash = (Get-FileHash "$installed/uninstall.exe").Hash
    New-Item "$installed/data" -ItemType Directory -Force | Out-Null
    Set-Content "$installed/data/legacy-preservation.txt" 'released 1.0.0 user state'
    $running = Start-Process "$installed/CodeConvoy.exe" -PassThru
    Start-Sleep -Seconds 3
    if ($running.HasExited) { throw 'Released application did not remain running' }
    if ($Overwrite) {
        $upgrade = Start-Process $next[0].FullName -ArgumentList "/P /D=$installed" -Wait -PassThru
    } else {
        # Observe the actual child uninstaller; successful file replacement
        # alone would not prove that the legacy runtime check was exercised.
        $eventName = "CodeConvoy-upgrade-$([guid]::NewGuid())"
        Register-CimIndicationEvent -Query 'SELECT * FROM Win32_ProcessStartTrace' -SourceIdentifier $eventName | Out-Null
        try {
            $upgrade = Start-Process $next[0].FullName -ArgumentList "/D=$installed" -PassThru
            $deadline = [DateTime]::UtcNow.AddSeconds(90)
            while (!$upgrade.HasExited -and [DateTime]::UtcNow -lt $deadline) {
                [CodeConvoyUpgradeUi]::Advance($upgrade.Id)
                # The unchanged 1.0.0 uninstaller can show its confirmation
                # page with /P. Drive that real child, never replace it.
                foreach ($child in @(Get-CimInstance Win32_Process -Filter "Name='uninstall.exe'" | Where-Object { $_.ParentProcessId -eq $upgrade.Id })) {
                    [CodeConvoyUpgradeUi]::Advance($child.ProcessId)
                }
                Start-Sleep -Milliseconds 200
            }
            if (!$upgrade.HasExited) { $upgrade.Kill(); throw 'Interactive legacy maintenance timed out' }
            $legacy = @(Get-Event -SourceIdentifier $eventName -ErrorAction SilentlyContinue | Where-Object {
                $_.SourceEventArgs.NewEvent.ProcessName -eq 'uninstall.exe' -and
                $_.SourceEventArgs.NewEvent.ParentProcessID -eq $upgrade.Id
            })
            if (!$legacy) { throw 'Upgrade did not invoke the actual released uninstaller' }
            Write-Output "Observed released 1.0.0 uninstaller child; original SHA256=$legacyHash"
        } finally {
            Unregister-Event -SourceIdentifier $eventName
            Get-Event -SourceIdentifier $eventName -ErrorAction SilentlyContinue | Remove-Event
        }
    }
    if ($upgrade.ExitCode -ne 0 -or !$running.WaitForExit(5000)) { throw 'Released upgrade/process handling failed' }
    if ((Get-ItemProperty $key).DisplayVersion -ne $config.version) { throw 'Upgrade version registration failed' }
    if ((Get-Content "$installed/data/legacy-preservation.txt") -ne 'released 1.0.0 user state') { throw 'Legacy user state changed' }
    # A real newer-version installation now exists. Downgrade using the actual
    # current installer, without fabricating a previous registry version.
    $downgrade = Start-Process $currentSetup -ArgumentList "/P /D=$installed" -Wait -PassThru
    if ($downgrade.ExitCode -ne 0 -or (Get-ItemProperty $key).DisplayVersion -ne $currentVersion.ToString()) {
        throw 'Real installer downgrade failed'
    }
    Write-Output "Released 1.0.0 -> $($config.version) -> $currentVersion passed; overwrite=$Overwrite"
}

# Drive only the test installer's standard NSIS buttons. No dependency on an
# automation package, screen coordinates, another desktop, or application code.
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class CodeConvoyUpgradeUi {
    private delegate bool EnumProc(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumProc callback, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumChildWindows(IntPtr parent, EnumProc callback, IntPtr data);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetWindowTextW(IntPtr window, StringBuilder text, int count);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] private static extern bool IsWindowEnabled(IntPtr window);
    [DllImport("user32.dll")] private static extern IntPtr SendMessageW(IntPtr window, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] private static extern bool PostMessageW(IntPtr window, uint msg, IntPtr w, IntPtr l);
    public static void Advance(int process) {
        EnumWindows((window, unused) => {
            uint pid;
            GetWindowThreadProcessId(window, out pid);
            if (pid != process || !IsWindowVisible(window)) return true;
            IntPtr next = IntPtr.Zero;
            EnumChildWindows(window, (child, ignored) => {
                if (!IsWindowVisible(child) || !IsWindowEnabled(child)) return true;
                var buffer = new StringBuilder(256);
                GetWindowTextW(child, buffer, buffer.Capacity);
                string text = buffer.ToString().Replace("&", "");
                if (text.StartsWith("Run CodeConvoy")) SendMessageW(child, 0xf1, IntPtr.Zero, IntPtr.Zero);
                if (text.StartsWith("Create desktop shortcut")) SendMessageW(child, 0xf1, new IntPtr(1), IntPtr.Zero);
                if (text == "Next >" || text == "I Agree" || text == "Install" || text == "Uninstall" || text == "Finish" || text == "Close") next = child;
                return true;
            }, IntPtr.Zero);
            if (next != IntPtr.Zero) PostMessageW(next, 0xf5, IntPtr.Zero, IntPtr.Zero);
            return true;
        }, IntPtr.Zero);
    }
}
'@
