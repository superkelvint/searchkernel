#!/usr/bin/env python3
"""Canonical Search Kernel consumer-runtime acquisition/cache contract."""
from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import platform as host_platform
import re
import shutil
import stat
import tarfile
import tempfile
import time
import urllib.parse
import urllib.request
from pathlib import Path

FORMAT_VERSION = 1
ENV_CACHE = "SEARCHKERNEL_RUNTIME_CACHE"
ENV_RUNTIME = "SEARCHKERNEL_RUNTIME_DIR"
ENV_ORIGIN = "SEARCHKERNEL_RUNTIME_ORIGIN"
IDENTITY_MATERIAL_KEYS = (
    "searchkernel_version",
    "searchkernel_revision",
    "runtime_package_generation",
    "platform",
    "native_target",
    "build_environment",
    "native_abi_version",
    "vespa_commit",
)
IDENTITY_KEYS = (*IDENTITY_MATERIAL_KEYS, "runtime_identity")
RUNTIME_IDENTITY_RE = re.compile(r"^skrt-v1-[0-9a-f]{64}$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
PLATFORM_TARGETS = {
    "linux-x86_64-glibc": "x86_64-unknown-linux-gnu",
}
ARTIFACT_SCHEMES = {"https", "file"}
ACQUIRE_TOTAL_TIMEOUT_SECONDS = 30.0
ACQUIRE_READ_TIMEOUT_SECONDS = 5.0


class RuntimeResolutionError(RuntimeError):
    def __init__(self, code: str, message: str):
        self.code = code
        super().__init__(f"{code}: {message}")


def canonical_identity_material(data: dict) -> bytes:
    material = {key: data[key] for key in IDENTITY_MATERIAL_KEYS}
    return json.dumps(material, sort_keys=True, separators=(",", ":")).encode()


def derive_runtime_identity(data: dict) -> str:
    return "skrt-v1-" + hashlib.sha256(canonical_identity_material(data)).hexdigest()


def current_platform() -> str:
    machine = host_platform.machine().lower()
    if os.name == "posix" and host_platform.system() == "Linux" and machine in {"x86_64", "amd64"}:
        libc_name, _ = host_platform.libc_ver()
        if libc_name.lower() in {"glibc", "gnu libc"}:
            return "linux-x86_64-glibc"
        try:
            glibc = os.confstr("CS_GNU_LIBC_VERSION")
        except (AttributeError, OSError, ValueError):
            glibc = None
        if glibc:
            return "linux-x86_64-glibc"
    raise RuntimeResolutionError(
        "platform-mismatch",
        f"unsupported consumer-runtime host: {host_platform.system()}/{machine}",
    )


def expected_artifact_filename(data: dict) -> str:
    return f"searchkernel-runtime-{data['runtime_identity']}-{data['platform']}.tar.gz"


def _validate_artifact_url(url: str, expected_filename: str) -> str:
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme not in ARTIFACT_SCHEMES:
        raise RuntimeResolutionError(
            "origin-invalid",
            f"artifact URL scheme {parsed.scheme!r} is not permitted",
        )
    if parsed.scheme == "https" and not parsed.netloc:
        raise RuntimeResolutionError("origin-invalid", "https artifact URL must include a host")
    filename = Path(urllib.parse.unquote(parsed.path)).name
    if filename != expected_filename:
        raise RuntimeResolutionError(
            "origin-invalid",
            f"artifact URL must resolve exact immutable filename {expected_filename}",
        )
    return url


def artifact_url(descriptor: dict, env=os.environ) -> str:
    expected_filename = expected_artifact_filename(descriptor)
    configured = env.get(ENV_ORIGIN)
    if not configured:
        return _validate_artifact_url(str(descriptor["artifact_url"]), expected_filename)

    parsed = urllib.parse.urlparse(configured)
    if parsed.scheme not in ARTIFACT_SCHEMES:
        raise RuntimeResolutionError(
            "origin-invalid",
            f"runtime origin scheme {parsed.scheme!r} is not permitted",
        )
    if parsed.scheme == "https" and not parsed.netloc:
        raise RuntimeResolutionError("origin-invalid", "https runtime origin must include a host")
    base = configured.rstrip("/") + "/"
    resolved = urllib.parse.urljoin(base, expected_filename)
    return _validate_artifact_url(resolved, expected_filename)


def validate_descriptor(data: dict) -> dict:
    if not isinstance(data, dict):
        raise RuntimeResolutionError("descriptor-invalid", "descriptor must be an object")
    required = {"format_version", "artifact_url", "artifact_sha256", *IDENTITY_KEYS}
    missing = sorted(required - data.keys())
    if data.get("format_version") != FORMAT_VERSION or missing:
        raise RuntimeResolutionError(
            "descriptor-invalid",
            f"unsupported format or missing fields: {', '.join(missing)}",
        )

    for key in (
        "searchkernel_version",
        "searchkernel_revision",
        "platform",
        "native_target",
        "build_environment",
        "vespa_commit",
    ):
        if not isinstance(data.get(key), str) or not data[key]:
            raise RuntimeResolutionError("descriptor-invalid", f"{key} must be a non-empty string")
    for key in ("runtime_package_generation", "native_abi_version"):
        if not isinstance(data.get(key), int) or isinstance(data.get(key), bool) or data[key] < 1:
            raise RuntimeResolutionError("descriptor-invalid", f"{key} must be a positive integer")

    identity = data.get("runtime_identity")
    if not isinstance(identity, str) or not RUNTIME_IDENTITY_RE.fullmatch(identity):
        raise RuntimeResolutionError("descriptor-invalid", "runtime_identity has invalid syntax")
    derived = derive_runtime_identity(data)
    if identity != derived:
        raise RuntimeResolutionError(
            "descriptor-invalid",
            f"runtime_identity does not match identity material: expected {derived}",
        )

    digest = data.get("artifact_sha256")
    if not isinstance(digest, str) or not SHA256_RE.fullmatch(digest):
        raise RuntimeResolutionError("descriptor-invalid", "artifact_sha256 must be lowercase SHA-256 hex")

    host = current_platform()
    if data["platform"] != host:
        raise RuntimeResolutionError(
            "platform-mismatch",
            f"descriptor platform {data['platform']} does not match qualified host {host}",
        )
    expected_target = PLATFORM_TARGETS.get(host)
    if data["native_target"] != expected_target:
        raise RuntimeResolutionError(
            "platform-mismatch",
            f"native target {data['native_target']} does not match {expected_target}",
        )

    artifact = data.get("artifact_url")
    if not isinstance(artifact, str) or not artifact:
        raise RuntimeResolutionError("origin-invalid", "artifact_url must be a non-empty string")
    _validate_artifact_url(artifact, expected_artifact_filename(data))
    return data


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def payload_digest(root: Path) -> str:
    h = hashlib.sha256()
    for path in sorted(
        p for p in root.rglob("*") if p.is_file() and p.name != "manifest.json"
    ):
        rel = path.relative_to(root).as_posix().encode()
        data = path.read_bytes()
        h.update(len(rel).to_bytes(8, "big"))
        h.update(rel)
        h.update(len(data).to_bytes(8, "big"))
        h.update(data)
    return h.hexdigest()


def load_descriptor(path: Path) -> dict:
    try:
        data = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise RuntimeResolutionError("descriptor-invalid", str(exc)) from exc
    return validate_descriptor(data)


def cache_root(env=os.environ) -> Path:
    if env.get(ENV_CACHE):
        return Path(env[ENV_CACHE]).expanduser()
    base = Path(env.get("XDG_CACHE_HOME") or (Path.home() / ".cache"))
    return base / "searchkernel" / "runtime"


def _validate_cached_symlinks(root: Path) -> None:
    base = root.resolve()
    for path in root.rglob("*"):
        if not path.is_symlink():
            continue
        try:
            target = path.resolve(strict=True)
        except OSError as exc:
            raise RuntimeResolutionError(
                "runtime-invalid", f"runtime symlink cannot be resolved: {path}: {exc}"
            ) from exc
        if base != target and base not in target.parents:
            raise RuntimeResolutionError(
                "runtime-invalid", f"runtime symlink escapes runtime root: {path}"
            )


def validate_runtime(root: Path, expected: dict) -> dict:
    validate_descriptor(expected)
    try:
        manifest = json.loads((root / "manifest.json").read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise RuntimeResolutionError(
            "runtime-invalid", f"manifest unavailable: {exc}"
        ) from exc

    if manifest.get("format_version") != FORMAT_VERSION:
        raise RuntimeResolutionError(
            "runtime-invalid",
            f"unsupported runtime manifest format {manifest.get('format_version')!r}",
        )
    for key in IDENTITY_KEYS:
        if manifest.get(key) != expected.get(key):
            raise RuntimeResolutionError("identity-mismatch", f"{key} differs")
    try:
        derived = derive_runtime_identity(manifest)
    except KeyError as exc:
        raise RuntimeResolutionError(
            "runtime-invalid", f"runtime manifest missing identity field {exc.args[0]}"
        ) from exc
    if manifest.get("runtime_identity") != derived:
        raise RuntimeResolutionError(
            "identity-mismatch", "runtime manifest identity is not derived from its material"
        )

    _validate_cached_symlinks(root)
    required = [
        root / "bin/searchkerneld",
        root / "bin/searchkerneld.real",
        root / "lib/libsearchkernel_vespa.so.2",
    ]
    if any(not path.is_file() for path in required):
        raise RuntimeResolutionError("runtime-invalid", "required runtime member missing")
    for executable in required[:2]:
        try:
            mode = executable.stat().st_mode
        except OSError as exc:
            raise RuntimeResolutionError(
                "runtime-invalid", f"required executable unavailable: {executable}: {exc}"
            ) from exc
        if not mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH):
            raise RuntimeResolutionError(
                "runtime-invalid",
                f"required runtime executable is not executable: {executable.relative_to(root)}",
            )

    if manifest.get("payload_sha256") != payload_digest(root):
        raise RuntimeResolutionError("digest-mismatch", "runtime payload digest differs")
    return manifest


def _safe_extract(archive: tarfile.TarFile, dest: Path) -> None:
    base = dest.resolve()
    members = archive.getmembers()
    link_names = {
        Path(member.name)
        for member in members
        if member.issym() or member.islnk()
    }
    for member in members:
        relative = Path(member.name)
        target = (dest / relative).resolve()
        if base != target and base not in target.parents:
            raise RuntimeResolutionError(
                "artifact-invalid", "archive path escapes runtime root"
            )
        if any(parent in link_names for parent in relative.parents):
            raise RuntimeResolutionError(
                "artifact-invalid", "archive member traverses an archive link"
            )
        if member.islnk():
            raise RuntimeResolutionError(
                "artifact-invalid", "archive hard links are not permitted"
            )
        if member.issym():
            link = (target.parent / member.linkname).resolve()
            if base != link and base not in link.parents:
                raise RuntimeResolutionError(
                    "artifact-invalid", "archive link escapes runtime root"
                )
        if member.isdev() or member.isfifo():
            raise RuntimeResolutionError(
                "artifact-invalid", "archive special files are not permitted"
            )
    archive.extractall(dest, filter="data")


def _download(url: str, dest: Path) -> None:
    deadline = time.monotonic() + ACQUIRE_TOTAL_TIMEOUT_SECONDS
    try:
        with urllib.request.urlopen(
            url, timeout=ACQUIRE_READ_TIMEOUT_SECONDS
        ) as src, dest.open("wb") as out:
            while True:
                if time.monotonic() >= deadline:
                    raise TimeoutError(
                        f"runtime acquisition exceeded {ACQUIRE_TOTAL_TIMEOUT_SECONDS:.0f}s"
                    )
                chunk = src.read(1024 * 1024)
                if not chunk:
                    break
                out.write(chunk)
    except Exception as exc:
        raise RuntimeResolutionError(
            "acquire-failed", f"could not acquire immutable runtime: {exc}"
        ) from exc


def _remove_entry(path: Path) -> None:
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return
    except OSError as exc:
        raise RuntimeResolutionError(
            "cache-io-failed", f"could not inspect corrupt cache entry {path}: {exc}"
        ) from exc
    try:
        if stat.S_ISDIR(mode) and not stat.S_ISLNK(mode):
            shutil.rmtree(path)
        else:
            path.unlink()
    except OSError as exc:
        raise RuntimeResolutionError(
            "cache-io-failed", f"could not remove corrupt cache entry {path}: {exc}"
        ) from exc


def ensure_runtime(
    descriptor: dict,
    *,
    root: Path | None = None,
    override: Path | None = None,
    offline: bool = False,
) -> Path:
    validate_descriptor(descriptor)
    if override is None and os.environ.get(ENV_RUNTIME):
        override = Path(os.environ[ENV_RUNTIME])
    if override is not None:
        validate_runtime(override, descriptor)
        return override.resolve()

    root = root or cache_root()
    identity = descriptor["runtime_identity"]
    final = root / "objects" / identity
    try:
        validate_runtime(final, descriptor)
        return final
    except RuntimeResolutionError:
        if offline:
            raise RuntimeResolutionError(
                "offline-miss", f"validated runtime {identity} is not cached"
            )

    try:
        root.mkdir(parents=True, exist_ok=True)
        lock_dir = root / "locks"
        lock_dir.mkdir(exist_ok=True)
        lock_path = lock_dir / f"{identity}.lock"
        with lock_path.open("a+") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            try:
                validate_runtime(final, descriptor)
                return final
            except RuntimeResolutionError:
                _remove_entry(final)
            if offline:
                raise RuntimeResolutionError(
                    "offline-miss", f"validated runtime {identity} is not cached"
                )

            staging_parent = root / "staging"
            staging_parent.mkdir(exist_ok=True)
            for stale in staging_parent.glob(f"{identity}.*"):
                _remove_entry(stale)

            with tempfile.TemporaryDirectory(
                prefix=f"{identity}.", dir=staging_parent
            ) as raw:
                temp = Path(raw)
                artifact = temp / "runtime.tar.gz"
                unpack = temp / "unpack"
                unpack.mkdir()
                _download(artifact_url(descriptor), artifact)
                if sha256_file(artifact) != descriptor["artifact_sha256"]:
                    raise RuntimeResolutionError(
                        "artifact-digest-mismatch",
                        "downloaded archive digest differs",
                    )
                try:
                    with tarfile.open(artifact, "r:gz") as archive:
                        _safe_extract(archive, unpack)
                except RuntimeResolutionError:
                    raise
                except (tarfile.TarError, OSError) as exc:
                    raise RuntimeResolutionError("artifact-invalid", str(exc)) from exc

                validate_runtime(unpack, descriptor)
                final.parent.mkdir(parents=True, exist_ok=True)
                os.rename(unpack, final)
        validate_runtime(final, descriptor)
        return final
    except RuntimeResolutionError:
        raise
    except OSError as exc:
        raise RuntimeResolutionError(
            "cache-io-failed", f"consumer runtime cache operation failed: {exc}"
        ) from exc


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("ensure", "prefetch", "validate"))
    parser.add_argument("descriptor", type=Path)
    parser.add_argument("--cache", type=Path)
    parser.add_argument("--runtime", type=Path)
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()

    if args.command == "validate" and not args.runtime:
        parser.error("validate requires --runtime")

    try:
        descriptor = load_descriptor(args.descriptor)
        if args.command == "validate":
            validate_runtime(args.runtime, descriptor)
            path = args.runtime
        else:
            path = ensure_runtime(
                descriptor,
                root=args.cache,
                override=args.runtime,
                offline=args.offline,
            )
    except RuntimeResolutionError as exc:
        print(str(exc), file=__import__("sys").stderr)
        return 2
    print(path.resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
