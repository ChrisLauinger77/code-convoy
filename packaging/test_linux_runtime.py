"""Window-manager readiness must precede the exact-package startup probe."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

import linux_runtime as runtime


class WindowManagerReadiness(unittest.TestCase):
    def test_advertised_manager_must_finish_startup_before_application_launch(self):
        with tempfile.TemporaryDirectory() as temporary:
            ready = Path(temporary) / "openbox-ready"
            process = Mock(returncode=None)
            process.poll.return_value = None
            delays = []

            def advance(delay):
                delays.append(delay)
                if len(delays) == 2:
                    ready.touch()

            # Even a successful wmctrl -m is insufficient. Only the actual
            # startup callback may release the wait; no wall-clock sleeps.
            with patch.object(runtime.subprocess, "run", return_value=Mock(returncode=0)), \
                    patch.object(runtime.time, "monotonic", return_value=0), \
                    patch.object(runtime.time, "sleep", side_effect=advance):
                runtime.wait_for_window_manager(process, ready)
            self.assertEqual(len(delays), 2)
            self.assertTrue(ready.exists())

    def test_missing_startup_callback_times_out(self):
        process = Mock(returncode=None)
        process.poll.return_value = None
        ready = Mock()
        ready.exists.return_value = False
        with patch.object(runtime.time, "monotonic", side_effect=[0, 0, 5, 10]), \
                patch.object(runtime.time, "sleep"), \
                self.assertRaisesRegex(RuntimeError, "did not become ready"):
            runtime.wait_for_window_manager(process, ready)

    def test_exited_manager_cannot_authorize_launch_even_with_marker(self):
        process = Mock(returncode=7)
        process.poll.return_value = 7
        ready = Mock()
        ready.exists.return_value = True
        with patch.object(runtime.time, "monotonic", return_value=0), \
                self.assertRaisesRegex(RuntimeError, "Window manager exited: 7"):
            runtime.wait_for_window_manager(process, ready)


if __name__ == "__main__":
    unittest.main()
