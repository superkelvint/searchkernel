"""Lifecycle tests for the Python client's owned local daemon."""

from __future__ import annotations

import gc
import os
import signal
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel import client as client_module  # noqa: E402


class _Process:
    pid = 321
    stdout = None
    stderr = None

    def __init__(self):
        self.returncode = None

    def poll(self):
        return self.returncode

    def wait(self, timeout):
        del timeout
        self.returncode = 0

    def send_signal(self, _signum):
        raise AssertionError("POSIX managed hosts should signal the process group")


class ManagedLocalHostTest(unittest.TestCase):
    def _host(self):
        temporary = tempfile.TemporaryDirectory(prefix="searchkernel-managed-host-test-")
        root = Path(temporary.name)
        binary = root / "searchkerneld"
        binary.touch()
        process = _Process()
        patches = (
            patch.object(client_module, "_host_binary", return_value=binary),
            patch.object(client_module.subprocess, "Popen", return_value=process),
            patch.object(client_module._ManagedLocalHost, "_wait_until_ready"),
        )
        for item in patches:
            item.start()

        def stop_patches():
            for item in reversed(patches):
                item.stop()

        self.addCleanup(stop_patches)
        host = client_module._ManagedLocalHost(root / "index", b"definition")
        self.addCleanup(temporary.cleanup)
        return host, process

    def test_daemon_output_cannot_block_and_process_group_is_owned(self):
        host, process = self._host()
        popen = client_module.subprocess.Popen

        popen.assert_called_once()
        self.assertEqual(popen.call_args.kwargs["stdout"], client_module.subprocess.DEVNULL)
        self.assertEqual(popen.call_args.kwargs["stderr"], client_module.subprocess.DEVNULL)
        self.assertTrue(popen.call_args.kwargs["start_new_session"])

        with patch.object(client_module.os, "killpg") as killpg:
            host.close()

        killpg.assert_called_once_with(process.pid, signal.SIGINT)
        self.assertIsNone(host.process)
        self.assertFalse(host._finalizer)

    def test_stale_legacy_sdk_directory_never_changes_runtime_loader_path(self):
        with patch.dict(os.environ, {"LD_LIBRARY_PATH": "/existing"}, clear=False):
            os.environ.pop("SEARCHKERNEL_SDK_LIB", None)
            with patch.object(client_module.Path, "is_dir", return_value=True):
                host, _process = self._host()

        popen = client_module.subprocess.Popen
        environment = popen.call_args.kwargs["env"]
        self.assertEqual(environment.get("LD_LIBRARY_PATH"), "/existing")
        host.close(force=True)

    def test_explicit_adapter_runtime_path_is_prepended_for_child_only(self):
        with patch.dict(
            os.environ,
            {
                "SEARCHKERNEL_SDK_LIB": "/exact/native-adapter/lib",
                "LD_LIBRARY_PATH": "/existing",
            },
            clear=False,
        ):
            host, _process = self._host()
            self.assertEqual(os.environ.get("LD_LIBRARY_PATH"), "/existing")

        popen = client_module.subprocess.Popen
        environment = popen.call_args.kwargs["env"]
        self.assertEqual(
            environment.get("LD_LIBRARY_PATH"),
            "/exact/native-adapter/lib:/existing",
        )
        host.close(force=True)

    def test_finalizer_stops_daemon_and_removes_owned_resources(self):
        host, process = self._host()
        definition_path = host.definition_path
        socket_directory = Path(host.socket_dir.name)
        finalizer = host._finalizer
        self.assertIsNotNone(finalizer)
        assert finalizer is not None

        with patch.object(client_module.os, "killpg") as killpg:
            del host
            gc.collect()

        killpg.assert_called_once_with(process.pid, signal.SIGINT)
        self.assertFalse(definition_path.exists())
        self.assertFalse(socket_directory.exists())
        self.assertFalse(finalizer.alive)


if __name__ == "__main__":
    unittest.main()
