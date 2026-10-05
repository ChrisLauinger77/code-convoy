"""Fresh packaged startup without agent CLIs; run under Xvfb on Linux.

This checks startup/liveness, not interactive desktop behavior. macOS interactive
checks are recorded separately. Only temporary per-process home/data paths change.
"""
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

binary = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="codeconvoy-package-smoke-") as temporary:
    home = Path(temporary) / "home"
    home.mkdir()
    env = dict(os.environ, HOME=str(home), XDG_DATA_HOME=str(home / "data"),
               XDG_CONFIG_HOME=str(home / "config"), PATH=os.defpath,
               APPIMAGE_EXTRACT_AND_RUN="1")
    with (Path(temporary) / "startup.log").open("w+b") as log:
        process = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=log,
                                   start_new_session=True)
        try:
            time.sleep(8)
            if process.poll() is not None:
                log.seek(0)
                raise RuntimeError(f"Packaged application exited during fresh startup ({process.returncode}):\n{log.read().decode(errors='replace')}")
            if not list(home.rglob("app.lock")):
                raise RuntimeError("Application did not initialize its isolated data directory")
            print("Fresh packaged startup passed: missing data directory, empty state, no agent CLIs.")
        finally:
            if process.poll() is None:
                # AppImage extraction may introduce a launcher child. Reap the
                # whole disposable session, not only its outer runtime process.
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
