#!/usr/bin/env python3
import importlib.util
import io
import json
import tarfile
import tempfile
import threading
import unittest
from unittest import mock
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "consumer_runtime", Path(__file__).with_name("consumer_runtime.py")
)
m = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m)

ROOT = Path(__file__).resolve().parents[1]
IDENTITY_MATERIAL = {
    "searchkernel_version": "0.1.0",
    "searchkernel_revision": "abc",
    "runtime_package_generation": 1,
    "platform": "linux-x86_64-glibc",
    "native_target": "x86_64-unknown-linux-gnu",
    "build_environment": "host",
    "native_abi_version": json.loads((ROOT / "reference/vespa/PINNED_VESPA.json").read_text())["native_abi_version"],
    "vespa_commit": "vespa",
}
IDENTITY = {
    **IDENTITY_MATERIAL,
    "runtime_identity": m.derive_runtime_identity(IDENTITY_MATERIAL),
}


def make_runtime(root: Path, identity: dict = IDENTITY) -> None:
    (root / "bin").mkdir(parents=True)
    (root / "lib").mkdir()
    (root / "bin/searchkerneld").write_text("launcher")
    (root / "bin/searchkerneld.real").write_bytes(b"daemon")
    (root / "bin/searchkerneld").chmod(0o755)
    (root / "bin/searchkerneld.real").chmod(0o755)
    (root / "lib/libsearchkernel_vespa.so.2").write_bytes(b"lib")
    (root / "manifest.json").write_text(
        json.dumps(
            {
                "format_version": 1,
                **identity,
                "payload_sha256": m.payload_digest(root),
            }
        )
    )


def make_descriptor(identity: dict, artifact: Path, digest: str) -> dict:
    return {
        "format_version": 1,
        **identity,
        "artifact_url": artifact.as_uri(),
        "artifact_sha256": digest,
    }


def make_artifact(base: Path, identity: dict = IDENTITY):
    runtime = base / "runtime"
    make_runtime(runtime, identity)
    artifact = base / m.expected_artifact_filename(identity)
    with tarfile.open(artifact, "w:gz") as tar:
        for name in ("manifest.json", "bin", "lib"):
            tar.add(runtime / name, arcname=name)
    descriptor = make_descriptor(identity, artifact, m.sha256_file(artifact))
    return descriptor, runtime, artifact


class Tests(unittest.TestCase):
    def test_contract_vector_derives_identity_and_cache_paths(self):
        contract = json.loads(
            (ROOT / "build/consumer-runtime/contract.json").read_text()
        )
        vectors = json.loads(
            (ROOT / "build/consumer-runtime/conformance.json").read_text()
        )["vectors"]
        self.assertEqual(contract["identity_material_fields"], list(m.IDENTITY_MATERIAL_KEYS))
        self.assertEqual(set(contract["error_codes"]), {
            "descriptor-invalid",
            "runtime-invalid",
            "identity-mismatch",
            "digest-mismatch",
            "platform-mismatch",
            "origin-invalid",
            "offline-miss",
            "acquire-failed",
            "artifact-digest-mismatch",
            "artifact-invalid",
            "cache-io-failed",
        })
        self.assertEqual(contract["runtime_origin_environment"], m.ENV_ORIGIN)
        for vector in vectors:
            identity = m.derive_runtime_identity(vector["identity_material"])
            self.assertEqual(identity, vector["runtime_identity"])
            self.assertEqual(vector["cache_relative_path"], f"objects/{identity}")
            self.assertEqual(vector["lock_relative_path"], f"locks/{identity}.lock")

    def test_descriptor_rejects_tampered_derived_identity(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            descriptor, _, _ = make_artifact(base)
            descriptor["runtime_identity"] = "skrt-v1-" + "b" * 64
            with self.assertRaisesRegex(m.RuntimeResolutionError, "descriptor-invalid"):
                m.validate_descriptor(descriptor)

    def test_descriptor_rejects_platform_mismatch(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            material = {
                **IDENTITY_MATERIAL,
                "platform": "linux-aarch64-glibc",
                "native_target": "aarch64-unknown-linux-gnu",
            }
            identity = {**material, "runtime_identity": m.derive_runtime_identity(material)}
            artifact = base / m.expected_artifact_filename(identity)
            artifact.write_bytes(b"x")
            descriptor = make_descriptor(identity, artifact, m.sha256_file(artifact))
            with self.assertRaisesRegex(m.RuntimeResolutionError, "platform-mismatch"):
                m.validate_descriptor(descriptor)

    def test_descriptor_rejects_non_exact_artifact_origin(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            descriptor, _, _ = make_artifact(base)
            descriptor["artifact_url"] = (base / "latest.tar.gz").as_uri()
            with self.assertRaisesRegex(m.RuntimeResolutionError, "origin-invalid"):
                m.validate_descriptor(descriptor)

    def test_origin_override_preserves_exact_artifact_filename(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            descriptor, _, _ = make_artifact(base)
            resolved = m.artifact_url(
                descriptor, {m.ENV_ORIGIN: "file:///tmp/staged"}
            )
            self.assertEqual(
                resolved,
                "file:///tmp/staged/" + m.expected_artifact_filename(descriptor),
            )

    def test_warm_cache_ignores_invalid_origin_override(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            descriptor, _, artifact = make_artifact(base)
            cache = base / "cache"
            got = m.ensure_runtime(descriptor, root=cache)
            artifact.unlink()
            with mock.patch.dict(
                "os.environ", {m.ENV_ORIGIN: "ftp://invalid.example/runtime"}, clear=False
            ):
                self.assertEqual(m.ensure_runtime(descriptor, root=cache), got)


    def test_cold_then_warm_offline(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            descriptor, _, artifact = make_artifact(base)
            cache = base / "cache"
            got = m.ensure_runtime(descriptor, root=cache)
            artifact.unlink()
            self.assertEqual(m.ensure_runtime(descriptor, root=cache), got)
            self.assertEqual(
                m.ensure_runtime(descriptor, root=cache, offline=True), got
            )

    def test_bad_archive_digest_fails_closed(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            descriptor, _, _ = make_artifact(base)
            descriptor["artifact_sha256"] = "0" * 64
            with self.assertRaisesRegex(
                m.RuntimeResolutionError, "artifact-digest-mismatch"
            ):
                m.ensure_runtime(descriptor, root=base / "cache")
            self.assertFalse(
                (base / "cache/objects" / IDENTITY["runtime_identity"]).exists()
            )

    def test_corrupt_cache_is_replaced(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            descriptor, _, _ = make_artifact(base)
            cache = base / "cache"
            final = cache / "objects" / IDENTITY["runtime_identity"]
            make_runtime(final)
            (final / "bin/searchkerneld.real").write_bytes(b"bad")
            self.assertEqual(m.ensure_runtime(descriptor, root=cache), final)
            m.validate_runtime(final, descriptor)

    def test_regular_file_corrupt_cache_is_replaced(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            descriptor, _, _ = make_artifact(base)
            cache = base / "cache"
            final = cache / "objects" / IDENTITY["runtime_identity"]
            final.parent.mkdir(parents=True)
            final.write_text("not a runtime")
            self.assertEqual(m.ensure_runtime(descriptor, root=cache), final)
            self.assertTrue(final.is_dir())
            m.validate_runtime(final, descriptor)

    def test_manifest_format_mismatch_fails_closed(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            descriptor, runtime, _ = make_artifact(base)
            manifest_path = runtime / "manifest.json"
            manifest = json.loads(manifest_path.read_text())
            manifest["format_version"] = 2
            manifest_path.write_text(json.dumps(manifest))
            with self.assertRaisesRegex(m.RuntimeResolutionError, "runtime-invalid"):
                m.validate_runtime(runtime, descriptor)

    def test_non_executable_daemon_fails_closed(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            descriptor, runtime, _ = make_artifact(base)
            (runtime / "bin/searchkerneld.real").chmod(0o644)
            with self.assertRaisesRegex(m.RuntimeResolutionError, "not executable"):
                m.validate_runtime(runtime, descriptor)

    def test_exact_preprovisioned_override_and_mismatch(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            descriptor, runtime, _ = make_artifact(base)
            self.assertEqual(
                m.ensure_runtime(descriptor, override=runtime), runtime.resolve()
            )

            changed_material = {**IDENTITY_MATERIAL, "native_abi_version": IDENTITY_MATERIAL["native_abi_version"] + 1}
            changed_identity = {
                **changed_material,
                "runtime_identity": m.derive_runtime_identity(changed_material),
            }
            artifact = base / m.expected_artifact_filename(changed_identity)
            artifact.write_bytes(b"placeholder")
            mismatch = make_descriptor(
                changed_identity, artifact, m.sha256_file(artifact)
            )
            with self.assertRaisesRegex(m.RuntimeResolutionError, "identity-mismatch"):
                m.ensure_runtime(mismatch, override=runtime)

    def test_concurrent_first_use_publishes_one_valid_object(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            descriptor, _, _ = make_artifact(base)
            cache = base / "cache"
            results = []
            errors = []

            def worker():
                try:
                    results.append(m.ensure_runtime(descriptor, root=cache))
                except Exception as exc:
                    errors.append(exc)

            threads = [threading.Thread(target=worker) for _ in range(4)]
            [thread.start() for thread in threads]
            [thread.join() for thread in threads]
            self.assertFalse(errors)
            self.assertEqual(len(set(results)), 1)
            self.assertEqual(
                [path.name for path in (cache / "objects").iterdir()],
                [IDENTITY["runtime_identity"]],
            )
            m.validate_runtime(results[0], descriptor)

    def test_interrupted_staging_is_cleaned_before_recovery(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            descriptor, _, _ = make_artifact(base)
            cache = base / "cache"
            stale = (
                cache
                / "staging"
                / (IDENTITY["runtime_identity"] + ".interrupted")
            )
            stale.mkdir(parents=True)
            (stale / "partial").write_bytes(b"partial")
            final = m.ensure_runtime(descriptor, root=cache)
            self.assertFalse(stale.exists())
            m.validate_runtime(final, descriptor)

    def test_archive_path_escape_fails_closed(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            artifact = base / m.expected_artifact_filename(IDENTITY)
            with tarfile.open(artifact, "w:gz") as tar:
                info = tarfile.TarInfo("../escape")
                data = b"nope"
                info.size = len(data)
                tar.addfile(info, io.BytesIO(data))
            descriptor = make_descriptor(
                IDENTITY, artifact, m.sha256_file(artifact)
            )
            with self.assertRaisesRegex(m.RuntimeResolutionError, "artifact-invalid"):
                m.ensure_runtime(descriptor, root=base / "cache")
            self.assertFalse((base / "escape").exists())

    def test_offline_miss_never_acquires(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            descriptor, _, artifact = make_artifact(base)
            artifact.unlink()
            with self.assertRaisesRegex(m.RuntimeResolutionError, "offline-miss"):
                m.ensure_runtime(descriptor, root=base / "cache", offline=True)


if __name__ == "__main__":
    unittest.main()
