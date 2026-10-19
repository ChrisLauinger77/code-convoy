# External Git clients

CodeConvoy can open a repository or worktree in the Git client you already use.
It starts the client as a separate program. It does not run Git, change the
repository, embed Git features or manage client accounts.

## Settings

Open **Settings → Git client…** and choose a client. Changes apply when you
select **Save**; **Cancel** or Escape keeps the saved configuration.

| Field | Presets | Custom |
| --- | --- | --- |
| Executable / Application | Optional. Blank means automatic detection. | Required: an absolute path or a command name. |
| Arguments | Fixed, verified per platform | One argument per line |

`{repo}` is replaced by the absolute path of the directory to open, in every
argument where it appears, including inside an argument such as `--path={repo}`.
Custom arguments must contain `{repo}`, so a client cannot silently show whatever
it opened last. Blank lines are ignored; all other characters, including spaces
and quotes, are passed unchanged. Arguments are never parsed by a shell.

The dialog shows **Ready** with the resolved command, or what is missing, for
the values being edited. Checks look at files only; they never start the client. On macOS, presets that
open an application accept its `.app` bundle; Sublime Merge also accepts its
bundle and then uses the `smerge` tool inside it.

Changing the client resets the executable and arguments. The preference is saved
in the existing local state file as an additive `git_client` field. It is omitted
while no client is configured, so older state files load unchanged. A client name
written by a newer CodeConvoy loads as not configured instead of blocking startup.

## Verified launch methods

Presets are shipped only where the launch method could be verified from the
client's documentation, release notes or source. Where it could not, the preset is
unavailable on that platform and **Custom** remains available.

| Client | Linux | macOS | Windows | Arguments |
| --- | --- | --- | --- | --- |
| GitKraken | `gitkraken` | Not offered | `gitkraken` | `{repo}` |
| SourceGit | `sourcegit`, `/opt/sourcegit/sourcegit` | `open -a SourceGit.app` | `sourcegit` | `{repo}` |
| AngKorGit | `angkorgit` | `open -a AngKorGit.app` | `angkorgit`, installer folders | `open {repo}` |
| Sublime Merge | `smerge`, `/opt/sublime_merge/sublime_merge` | `smerge` in the app bundle | `smerge`, `Program Files\Sublime Merge\smerge.exe` | `{repo}` |
| Fork | Not offered | `open -a Fork.app` | `fork`, `%LOCALAPPDATA%\Fork\Fork.exe` | `{repo}` |
| Anchorpoint | Not offered | Not offered | Not offered | — |

Command names are searched in `PATH` plus `~/.local/bin`, `~/bin`, Homebrew,
`/usr/local/bin`, `/usr/bin`, `/bin` and `/snap/bin` on Unix, and Scoop shims and
WinGet links on Windows. Application bundles are searched in `/Applications` and
`~/Applications`. A configured path is used exactly as entered.

Sources, as checked in October 2026:

- **GitKraken:** the Desktop documentation (*Open, Clone, and Init a Repo*) and
  12.1.0 release notes document `gitkraken /path/to/repo`. Earlier releases needed
  the `--path` flag; configure that through Custom if required. The installation
  guide documents the `gitkraken` terminal command on Linux and its Snap package.
  It documents no terminal command or folder-open behavior for macOS, so macOS is
  not offered. The Windows executable location is not documented: CodeConvoy finds
  `gitkraken` on `PATH`; otherwise enter the executable path.
- **SourceGit:** the README documents `<SOURCEGIT_EXEC> <DIR>` and support for
  Windows, macOS and Linux. Its Linux packages install `/opt/sourcegit/sourcegit`
  and `/usr/bin/sourcegit`. Its source opens a folder passed through a macOS
  open-document event, which `open -a` sends. Windows builds are portable, so
  enter the executable path if `sourcegit` is not on `PATH`.
- **AngKorGit:** the CLI help shipped in its source documents `angkorgit open
  [path]`. The application binary and the optional `angkorgit` CLI shim both accept
  it. On macOS its own shim uses `open -a`. The Windows folders are the Tauri
  installer defaults, `%LOCALAPPDATA%\AngKorGit` (per-user) and
  `%ProgramFiles%\AngKorGit` (MSI). On Windows the shim is a `.cmd` file, which
  CodeConvoy does not start.
- **Sublime Merge:** the *Command Line* documentation lists `smerge path`, the
  macOS launcher at `Sublime Merge.app/Contents/SharedSupport/bin/smerge`, the
  Windows folder `C:\Program Files\Sublime Merge` and the Linux tarball binary
  `/opt/sublime_merge/sublime_merge`.
- **Fork:** the macOS release notes document `open /path/to/repository -a
  /Applications/Fork.app` and `fork path/to/repo`. The Windows 1.23 release notes
  document `fork.exe path/to/repo`, and the maintainer documents
  `%localappdata%\fork\fork.exe /absolute/path/to/repo`. Fork has no Linux build.
- **Anchorpoint:** its documented `ap` CLI runs project commands and `connect`
  attaches to a running app; none opens a folder in the desktop application. No
  preset is shipped. Use Custom only with a launch method you have verified.

## Process behavior

- The client program and every argument are passed directly to the operating
  system as separate values. No shell, `cmd.exe` or PowerShell is involved, so
  spaces, quotes, `$()`, `;`, `&` and Unicode in paths have no special meaning.
- On Windows, only `.exe` files are started. `.cmd`, `.bat`, `.ps1` and shortcuts
  are rejected, because Windows would run scripts through a shell.
- Relative executable paths are rejected; command names are searched only in the
  absolute directories above, never in the current directory.
- The client starts in the directory being opened. CodeConvoy's own working
  directory is unchanged.
- The environment is inherited, including the desktop session (`DISPLAY`,
  `WAYLAND_DISPLAY`, `XDG_*`, `DBUS_SESSION_BUS_ADDRESS`). `GIT_*` variables are
  removed, as for every other process CodeConvoy starts in a repository, so they
  cannot redirect the client to another repository. When CodeConvoy runs as an
  AppImage, the runtime's own `APPIMAGE`, `APPDIR`, `ARGV0` and `OWD` are removed
  too.
- On macOS, `open -a` hands the folder to the application through
  LaunchServices; an application started that way uses the session environment.
- The client runs in its own process group (Unix) or without a console window in
  a new process group (Windows). It is never cancelled, supervised or stopped,
  including when CodeConvoy quits, and it holds no CodeConvoy repository lock.
- Launchers that hand off and exit, such as `open` or `smerge`, are reported as
  opened when they succeed. If the started program exits with an error within
  two seconds, its exit status is reported. A program still running after two
  seconds is reported as opened. Client output is not captured.
- Opening checks that the target is an existing absolute directory. A missing
  directory is reported; nothing falls back to another directory.

## Limitations

- SourceGit trims surrounding spaces and quote characters from its path argument,
  so a directory whose name starts or ends with a space cannot be opened by it.
- Snap- or Flatpak-confined clients can only open directories their sandbox
  permits. Flatpak installations are not detected; use Custom.
- A `.app` bundle chosen on macOS is passed to `open -a` without checking which
  application it contains.
- CodeConvoy cannot confirm that a client actually displayed the directory: a
  launcher's successful exit only shows that the request was handed off.

## Validation

Automated tests use a probe executable in place of a client on Linux, macOS and
Windows CI. They cover preset resolution and platform availability, custom
commands, missing and invalid executables, rejected Windows scripts, `{repo}`
substitution, argument boundaries with spaces, Unicode and shell metacharacters,
non-Unicode Unix paths, exit-status reporting, a client that keeps running,
missing directories and state compatibility. No installed client is needed in CI.

Native check on macOS 27.0.1 (Apple silicon), October 2026, through CodeConvoy's
launcher with disposable repositories whose paths contained spaces, `ü`, CJK
characters and `$(x)`:

| Client | Result |
| --- | --- |
| SourceGit 2026.22 (Homebrew) | Opened when not running and when already running |
| AngKorGit 0.24.0 | Opened in the running instance |
| Fork, Sublime Merge | Not installed; reported as not found |
| GitKraken | Not offered on macOS |

Each client recorded the exact path in its recent-repository list. `git status`
and `HEAD` of both repositories were unchanged and no files were added.

Not yet tested with an installed client: GitKraken, Sublime Merge and Fork on any
platform, and every client on Linux and Windows.
