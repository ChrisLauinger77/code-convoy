"""Exact-package X11 startup and normal window-close check on a private X server.

Run under xvfb-run and dbus-run-session, with openbox and wmctrl installed.
This is virtual-display evidence, not native desktop/Wayland certification.
"""
import argparse
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time


def stop(process):
    if process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=10)


def smoke(binary, mode):
    with tempfile.TemporaryDirectory(prefix="codeconvoy-linux-runtime-") as temporary:
        home = Path(temporary)
        env = dict(os.environ, HOME=str(home), XDG_DATA_HOME=str(home / "data"),
                   XDG_CONFIG_HOME=str(home / "config"), XDG_RUNTIME_DIR=str(home / "runtime"),
                   LIBGL_ALWAYS_SOFTWARE="1")
        (home / "runtime").mkdir(mode=0o700)
        for key in ("WAYLAND_DISPLAY", "WAYLAND_SOCKET", "APPIMAGE_EXTRACT_AND_RUN"):
            env.pop(key, None)
        if mode == "extract":
            env["APPIMAGE_EXTRACT_AND_RUN"] = "1"
        mounts_before = set(Path("/proc/self/mountinfo").read_text().splitlines())
        with (home / "startup.log").open("w+b") as log:
            wm = subprocess.Popen(["openbox", "--sm-disable"], env=env, stdout=log, stderr=log, start_new_session=True)
            process = None
            try:
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    if wm.poll() is not None:
                        raise RuntimeError(f"Window manager exited: {wm.returncode}")
                    if subprocess.run(["wmctrl", "-m"], env=env, capture_output=True).returncode == 0:
                        break
                    time.sleep(0.1)
                else:
                    raise RuntimeError("Window manager did not become ready")
                process = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=log, start_new_session=True)
                deadline = time.monotonic() + 30
                window = None
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise RuntimeError(f"Application exited during startup: {process.returncode}")
                    listing = subprocess.run(["wmctrl", "-l"], env=env, capture_output=True, text=True)
                    window = next((line.split()[0] for line in listing.stdout.splitlines() if "CodeConvoy" in line), None)
                    if window and list(home.rglob("app.lock")):
                        break
                    time.sleep(0.1)
                else:
                    raise RuntimeError("Application did not create its window and isolated store")
                time.sleep(2)  # Allow startup/render work to run before closing.
                if process.poll() is not None:
                    raise RuntimeError("Application exited after creating its window")
                if mode == "fuse":
                    mounts = set(Path("/proc/self/mountinfo").read_text().splitlines()) - mounts_before
                    if not any(" - fuse" in line and ".mount_" in line for line in mounts):
                        raise RuntimeError("FUSE launch did not create an AppImage mount")
                # A real window-manager close exercises the ordinary application
                # shutdown path. Signals are reserved for failed-test cleanup.
                subprocess.run(["wmctrl", "-ic", window], env=env, check=True)
                if process.wait(timeout=30) != 0:
                    raise RuntimeError(f"Application failed normal shutdown: {process.returncode}")
                print(f"PASS {binary.name}: {mode}, Xvfb window/store initialization and normal close", flush=True)
            except Exception:
                for command in (["wmctrl", "-lp"], ["ps", "-eLf"]):
                    diagnostic = subprocess.run(command, env=env, capture_output=True, text=True)
                    print(f"{command}:\n{diagnostic.stdout}{diagnostic.stderr}", flush=True)
                print(f"Isolated store locks: {list(home.rglob('app.lock'))}", flush=True)
                log.flush()
                log.seek(0)
                print(log.read().decode(errors="replace"), flush=True)
                raise
            finally:
                if process is not None:
                    stop(process)
                stop(wm)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--mode", choices=["native", "extract", "fuse"], default="native")
    args = parser.parse_args()
    smoke(args.binary.resolve(strict=True), args.mode)
