"""User-supplied Windows runtime policy; never downloads or stages Microsoft DLLs."""
import json
from pathlib import Path, PureWindowsPath
import re

ROOT = Path(__file__).resolve().parent.parent
POLICY = ROOT / "packaging/windows-runtime.json"


def policy():
    return json.loads(POLICY.read_text())


def reject_bundled_runtimes(names):
    for name in names:
        basename = PureWindowsPath(name).name.lower()
        if re.match(r"(?:vcruntime|msvcp|msvcr|ucrtbase|api-ms-win-crt-).*\.dll$", basename) or re.match(
                r"(?:vc[_-]?redist|vcredist).*\.exe$", basename):
            raise ValueError(f"Microsoft runtime redistribution is prohibited: {name}")


def render_prerequisites(spec=None):
    spec = spec or policy()
    text = (ROOT / "packaging/nsis/prerequisites.nsh").read_text()
    for architecture in ("x86", "x64"):
        checks = []
        for name in spec["runtime_files"][architecture]:
            version = spec["minimum_ucrt_version" if name == "ucrtbase.dll" else "minimum_vc_version"]
            checks += [f'  StrCpy $CrtFile "$CrtDirectory\\{name}"',
                       f'  StrCpy $CrtMinimumMS {version[0] * 65536 + version[1]}',
                       f'  StrCpy $CrtMinimumLS {version[2] * 65536 + version[3]}',
                       '  Call ${PREFIX}CheckRuntimeFile',
                       '  ${If} $CrtFileOK != 1',
                       '    StrCpy $CrtArchitectureOK 0',
                       '  ${EndIf}']
        text = text.replace(f"@CHECKS_{architecture.upper()}@", "\n".join(checks))
    return text.replace("@DOWNLOAD_PAGE@", spec["download_page"]).replace(
        "@VC_VERSION@", ".".join(map(str, spec["minimum_vc_version"])))


def capture_system(directory):
    """Record the runner's real files; these files are never copied to artifacts."""
    import subprocess
    script = r'''
$ErrorActionPreference = 'Stop'
$files = foreach ($arch in @('x64', 'x86')) {
  $folder = if ($arch -eq 'x64') { 'System32' } else { 'SysWOW64' }
  foreach ($name in @('vcruntime140.dll', 'vcruntime140_1.dll', 'ucrtbase.dll')) {
    $path = Join-Path $env:SystemRoot "$folder/$name"
    if (Test-Path $path) {
      $v = (Get-Item $path).VersionInfo
      @{ architecture=$arch; name=$name; path=$path; sha256=(Get-FileHash $path).Hash.ToLower();
         version=@($v.FileMajorPart,$v.FileMinorPart,$v.FileBuildPart,$v.FilePrivatePart) }
    }
  }
}
ConvertTo-Json -Depth 4 -InputObject @($files)
'''
    result = subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", script],
                            check=True, capture_output=True, text=True)
    (directory / "system-runtime-files.json").write_text(
        json.dumps(json.loads(result.stdout), indent=2) + "\n")
