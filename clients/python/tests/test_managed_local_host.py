"""Lifecycle tests for the Python client's owned local daemon."""

from __future__ import annotations

import gc
import os
import signal
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel import client as client_module  # noqa: E402


class HostBinaryResolutionTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="searchkernel-host-resolution-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        source = self.root / "clients/python/searchkernel/client.py"
        source.parent.mkdir(parents=True)
        source.touch()
        environment = patch.dict(os.environ, {}, clear=False)
        environment.start()
        self.addCleanup(environment.stop)
        os.environ.pop("SEARCHKERNEL_LOCAL_HOST", None)
        os.environ.pop("CARGO_TARGET_DIR", None)
        location = patch.object(client_module, "__file__", str(source))
        location.start()
        self.addCleanup(location.stop)

    def binary(self, relative, output):
        binary = self.root / relative
        binary.parent.mkdir(parents=True, exist_ok=True)
        binary.write_text(f"#!{sys.executable}\nprint({output!r})\n", encoding="utf-8")
        binary.chmod(0o755)
        return binary

    def test_default_launches_active_build_instead_of_stale_legacy_binary(self):
        self.binary("target/debug/searchkerneld", "STALE")
        expected = self.binary("target/host/debug/searchkerneld", "CURRENT")
        self.assertEqual(client_module._host_binary(), expected)
        result = subprocess.run(
            [str(client_module._host_binary())], capture_output=True, text=True, check=True
        )
        self.assertEqual(result.stdout.strip(), "CURRENT")

    def test_absolute_and_relative_cargo_targets(self):
        for target in ("target/isolated", str(self.root / "absolute-target")):
            with self.subTest(target=target), patch.dict(os.environ, {"CARGO_TARGET_DIR": target}):
                expected = self.binary(Path(target) / "debug/searchkerneld", target)
                self.assertEqual(client_module._host_binary(), expected)

    def test_missing_active_build_does_not_fall_back_to_stale_binary(self):
        self.binary("target/debug/searchkerneld", "STALE")
        with self.assertRaisesRegex(client_module.ClientError, "target/host/debug/searchkerneld"):
            client_module._ManagedLocalHost(self.root / "data")

    def test_explicit_then_packaged_binary_precede_repository_fallback(self):
        packaged = self.binary("clients/python/searchkernel/bin/searchkerneld", "PACKAGED")
        explicit = self.binary("explicit/searchkerneld", "EXPLICIT")
        with patch.dict(os.environ, {"SEARCHKERNEL_LOCAL_HOST": str(explicit)}):
            self.assertEqual(client_module._host_binary(), explicit)
        self.assertEqual(client_module._host_binary(), packaged)


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
        process.stdout = tempfile.TemporaryFile()
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
        host = client_module._ManagedLocalHost(root / "data-root")
        self.addCleanup(temporary.cleanup)
        return host, process

    def test_daemon_output_cannot_block_and_process_group_is_owned(self):
        host, process = self._host()
        popen = client_module.subprocess.Popen

        popen.assert_called_once()
        command = popen.call_args.args[0]
        self.assertIn("--data-root", command)
        self.assertNotIn("--index", command)
        self.assertNotIn("--definition", command)
        self.assertEqual(popen.call_args.kwargs["stdout"], client_module.subprocess.PIPE)
        self.assertEqual(popen.call_args.kwargs["stderr"], client_module.subprocess.STDOUT)
        self.assertTrue(popen.call_args.kwargs["start_new_session"])
        capture = host._output_capture
        self.assertIsNotNone(capture)
        assert capture is not None
        self.assertTrue(capture._thread.daemon)

        with patch.object(client_module.os, "killpg") as killpg:
            host.close()

        killpg.assert_called_once_with(process.pid, signal.SIGINT)
        self.assertTrue(process.stdout.closed)
        self.assertIsNone(host.process)
        self.assertFalse(host._finalizer)

    def test_output_capture_drains_more_than_pipe_capacity_without_blocking(self):
        process = client_module.subprocess.Popen(
            [
                sys.executable,
                "-c",
                (
                    "import sys; "
                    "sys.stdout.buffer.write("
                    "b'BEGIN SHOULD BE TRUNCATED\\n' + b'x' * 262144 + b'\\nPIPE TAIL\\n'"
                    "); "
                    "sys.stdout.buffer.flush()"
                ),
            ],
            stdout=client_module.subprocess.PIPE,
            stderr=client_module.subprocess.STDOUT,
        )
        self.assertIsNotNone(process.stdout)
        assert process.stdout is not None
        capture = client_module._ManagedHostOutputCapture(process.stdout)
        try:
            self.assertEqual(process.wait(timeout=5), 0)
            output = capture.tail(wait_for_eof=True)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)
            capture.close()

        self.assertIn("PIPE TAIL", output)
        self.assertNotIn("BEGIN SHOULD BE TRUNCATED", output)
        self.assertLess(
            len(output.encode("utf-8")),
            client_module._MANAGED_HOST_OUTPUT_TAIL_BYTES + 128,
        )

    def test_startup_wait_allows_listener_after_old_fifteen_second_deadline(self):
        host = object.__new__(client_module._ManagedLocalHost)
        host.process = _Process()
        host.socket_path = Path("/tmp/searchkernel-managed-host-late.sock")
        host._output_capture = None

        class _Socket:
            attempts = 0

            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def settimeout(self, _timeout):
                return None

            def connect(self, _path):
                type(self).attempts += 1
                if type(self).attempts == 1:
                    raise OSError("listener not ready yet")

        with (
            patch.object(client_module.socket, "socket", side_effect=lambda *_args: _Socket()),
            patch.object(client_module.time, "monotonic", side_effect=[0.0, 0.0, 16.0]),
            patch.object(client_module.time, "sleep"),
        ):
            host._wait_until_ready()

        self.assertEqual(_Socket.attempts, 2)

    def test_startup_exit_reports_exit_code_and_bounded_daemon_output(self):
        temporary = tempfile.TemporaryDirectory(prefix="searchkernel-managed-host-failure-")
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        binary = root / "searchkerneld"
        binary.touch()
        process = _Process()
        process.returncode = 127
        captured = {}

        def popen(*_args, **kwargs):
            self.assertEqual(kwargs["stdout"], client_module.subprocess.PIPE)
            output = tempfile.TemporaryFile()
            output.write(
                b"BEGIN SHOULD BE TRUNCATED\n"
                + (b"x" * 20000)
                + b"\nsearchkerneld: error while loading shared libraries: "
                + b"libvespa_searchlib.so: cannot open shared object file\n"
            )
            output.seek(0)
            process.stdout = output
            captured["output"] = output
            return process

        with (
            patch.object(client_module, "_host_binary", return_value=binary),
            patch.object(client_module.subprocess, "Popen", side_effect=popen),
            self.assertRaises(client_module.ClientError) as raised,
        ):
            client_module._ManagedLocalHost(root / "data-root")

        message = str(raised.exception)
        self.assertIn("exit code 127", message)
        self.assertIn("libvespa_searchlib.so", message)
        self.assertNotIn("BEGIN SHOULD BE TRUNCATED", message)
        self.assertLess(
            len(message.encode("utf-8")),
            client_module._MANAGED_HOST_OUTPUT_TAIL_BYTES + 512,
        )
        self.assertTrue(captured["output"].closed)

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

    def test_managed_root_first_use_is_atomic_under_concurrency(self):
        temporary = tempfile.TemporaryDirectory(prefix="searchkernel-managed-root-race-")
        self.addCleanup(temporary.cleanup)
        data_root = Path(temporary.name) / ".searchkernel-managed"
        barrier = threading.Barrier(8)
        failures = []

        def prepare():
            try:
                barrier.wait()
                client_module._prepare_managed_data_root(data_root)
            except Exception as exc:  # pragma: no cover - asserted below
                failures.append(exc)

        threads = [threading.Thread(target=prepare) for _ in range(8)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()

        self.assertEqual(failures, [])
        self.assertEqual(
            (data_root / client_module._MANAGED_ROOT_MARKER_NAME).read_text(encoding="utf-8"),
            client_module._MANAGED_ROOT_MARKER_VALUE + "\n",
        )

    def test_managed_identity_marker_publication_converges(self):
        temporary = tempfile.TemporaryDirectory(prefix="searchkernel-managed-marker-race-")
        self.addCleanup(temporary.cleanup)
        identity = Path(temporary.name) / "legacy-index"
        barrier = threading.Barrier(8)
        failures = []

        def publish():
            try:
                barrier.wait()
                client_module._write_managed_marker(identity, "local-test")
            except Exception as exc:  # pragma: no cover - asserted below
                failures.append(exc)

        threads = [threading.Thread(target=publish) for _ in range(8)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()

        self.assertEqual(failures, [])
        self.assertEqual(
            (identity / client_module._MANAGED_MARKER_NAME).read_text(encoding="utf-8"),
            "local-test\n",
        )

    def test_managed_identity_marker_waits_for_inflight_publication(self):
        temporary = tempfile.TemporaryDirectory(prefix="searchkernel-managed-marker-inflight-")
        self.addCleanup(temporary.cleanup)
        identity = Path(temporary.name) / "legacy-index"
        marker = identity / client_module._MANAGED_MARKER_NAME
        marker_created = threading.Event()
        empty_observed = threading.Event()
        release_writer = threading.Event()
        failures = []
        original_open = client_module.Path.open
        original_read_text = client_module.Path.read_text
        delayed_once = threading.Event()

        def delayed_open(path, *args, **kwargs):
            stream = original_open(path, *args, **kwargs)
            mode = args[0] if args else kwargs.get("mode", "r")
            if path == marker and mode == "x" and not delayed_once.is_set():
                delayed_once.set()
                marker_created.set()
                release_writer.wait(timeout=2)
            return stream

        def observed_read_text(path, *args, **kwargs):
            value = original_read_text(path, *args, **kwargs)
            if path == marker and value == "":
                empty_observed.set()
            return value

        def publish():
            try:
                client_module._write_managed_marker(identity, "local-test")
            except Exception as exc:  # pragma: no cover - asserted below
                failures.append(exc)

        with (
            patch.object(client_module.Path, "open", delayed_open),
            patch.object(client_module.Path, "read_text", observed_read_text),
        ):
            first = threading.Thread(target=publish)
            first.start()
            self.assertTrue(marker_created.wait(timeout=1))
            second = threading.Thread(target=publish)
            second.start()
            observed = empty_observed.wait(timeout=1)
            release_writer.set()
            first.join(timeout=2)
            second.join(timeout=2)

        self.assertTrue(observed, "the competing publisher never observed the in-flight marker")
        self.assertFalse(first.is_alive())
        self.assertFalse(second.is_alive())
        self.assertEqual(failures, [])
        self.assertEqual(marker.read_text(encoding="utf-8"), "local-test\n")

    def test_preexisting_empty_unowned_root_is_never_replaced(self):
        temporary = tempfile.TemporaryDirectory(prefix="searchkernel-managed-unowned-empty-")
        self.addCleanup(temporary.cleanup)
        data_root = Path(temporary.name) / ".searchkernel-managed"
        data_root.mkdir()
        inode = data_root.stat().st_ino

        with self.assertRaisesRegex(client_module.ClientError, "not owned"):
            client_module._prepare_managed_data_root(data_root)

        self.assertTrue(data_root.is_dir())
        self.assertEqual(data_root.stat().st_ino, inode)
        self.assertEqual(list(data_root.iterdir()), [])

    def test_managed_root_marker_type_and_version_are_validated(self):
        temporary = tempfile.TemporaryDirectory(prefix="searchkernel-managed-root-marker-")
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        data_root = root / ".searchkernel-managed"
        data_root.mkdir()
        marker = data_root / client_module._MANAGED_ROOT_MARKER_NAME

        marker.write_text("searchkernel-managed-v2\n", encoding="utf-8")
        with self.assertRaisesRegex(client_module.ClientError, "not owned"):
            client_module._prepare_managed_data_root(data_root)

        marker.unlink()
        marker.mkdir()
        with self.assertRaisesRegex(client_module.ClientError, "not owned"):
            client_module._prepare_managed_data_root(data_root)

        marker.rmdir()
        marker.write_text(client_module._MANAGED_ROOT_MARKER_VALUE + "\n", encoding="utf-8")
        client_module._prepare_managed_data_root(data_root)

    @unittest.skipUnless(hasattr(os, "symlink"), "symlinks unavailable")
    def test_legacy_migration_rejects_symlink_source_without_spawning_daemon(self):
        temporary = tempfile.TemporaryDirectory(prefix="searchkernel-managed-symlink-")
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        real = root / "real-index"
        real.mkdir()
        source = root / "legacy-index"
        source.symlink_to(real, target_is_directory=True)
        data_root = root / ".searchkernel-managed"
        client_module._prepare_managed_data_root(data_root)

        with (
            patch.object(client_module.subprocess, "run") as run,
            self.assertRaisesRegex(client_module.ClientError, "real directory"),
        ):
            client_module._migrate_legacy_managed_path(source, data_root, "local-test")
        run.assert_not_called()
        self.assertTrue(source.is_symlink())
        self.assertTrue(real.is_dir())

    def test_managed_host_scrubs_legacy_mode_environment(self):
        with patch.dict(
            os.environ,
            {
                "SEARCHKERNEL_INDEX_PATH": "/legacy/index",
                "SEARCHKERNEL_INDEX_DEFINITION": "/legacy/definition.fb",
                "SEARCHKERNEL_INDEX_DEFINITION_JSON": "/legacy/definition.json",
            },
            clear=False,
        ):
            environment = client_module._managed_host_environment()

        self.assertNotIn("SEARCHKERNEL_INDEX_PATH", environment)
        self.assertNotIn("SEARCHKERNEL_INDEX_DEFINITION", environment)
        self.assertNotIn("SEARCHKERNEL_INDEX_DEFINITION_JSON", environment)

    def test_finalizer_stops_daemon_and_removes_owned_resources(self):
        host, process = self._host()
        socket_directory = Path(host.socket_dir.name)
        output = process.stdout
        finalizer = host._finalizer
        self.assertIsNotNone(finalizer)
        assert finalizer is not None

        with patch.object(client_module.os, "killpg") as killpg:
            del host
            gc.collect()

        killpg.assert_called_once_with(process.pid, signal.SIGINT)
        self.assertFalse(socket_directory.exists())
        self.assertTrue(output.closed)
        self.assertFalse(finalizer.alive)


if __name__ == "__main__":
    unittest.main()
