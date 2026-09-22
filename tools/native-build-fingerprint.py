#!/usr/bin/env python3
"""Create a deterministic fingerprint for Search Kernel native artifacts."""

from __future__ import annotations

import argparse
import hashlib
import os
import subprocess
from pathlib import Path


VERSION = "3"


def command_output(program: str, *args: str) -> str:
    try:
        result = subprocess.run(
            [program, *args],
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as error:
        return f"error={error}"
    return (
        f"status={result.returncode}\n"
        f"stdout={result.stdout}\n"
        f"stderr={result.stderr}"
    )


def resolve_program(requested: str) -> Path:
    path = Path(requested)
    if path.is_absolute():
        return path.resolve(strict=False)
    for directory in os.environ.get("PATH", "").split(os.pathsep):
        candidate = Path(directory or ".") / path
        if candidate.is_file():
            return candidate.resolve()
    return path


def tool_identity(variable: str, default: str, dumpmachine: bool = False) -> str:
    requested = os.environ.get(variable, default)
    path = resolve_program(requested)
    machine = command_output(str(path), "-dumpmachine") if dumpmachine else ""
    return (
        f"path={path}\n"
        f"version={command_output(str(path), '--version')}\n"
        f"dumpmachine={machine}"
    )


def file_digest(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def source_digest(root: Path) -> str:
    source = root / "core/native/vespa"
    digest = hashlib.sha256()
    for path in sorted(path for path in source.rglob("*") if path.is_file()):
        if ".git" in path.parts:
            continue
        digest.update(path.relative_to(root).as_posix().encode())
        digest.update(b"\0")
        digest.update(file_digest(path).encode())
        digest.update(b"\n")
    return digest.hexdigest()


def shared_library_tree_identity(root: Path, pattern: str = "*.so*") -> str:
    """Content identity for packaged shared-library inputs.

    Cache identity must be stable across Git worktrees, so timestamps and inode
    metadata are deliberately excluded. Because SDK packaging dereferences
    shared-library symlinks, valid symlinks contribute the bytes that will
    actually be packaged under each library name.
    """
    if not root.is_dir():
        return "missing"
    digest = hashlib.sha256()
    for path in sorted(root.rglob(pattern)):
        relative = path.relative_to(root).as_posix()
        digest.update(relative.encode())
        digest.update(b"\0")
        try:
            if path.is_file():
                digest.update(b"file\0")
                digest.update(file_digest(path).encode())
            elif path.is_symlink():
                digest.update(b"broken-symlink\0")
                digest.update(os.readlink(path).encode())
            else:
                digest.update(b"other")
        except OSError as error:
            digest.update(f"error={error}".encode())
        digest.update(b"\n")
    return digest.hexdigest()


def runtime_bundle_identity(root: Path, vespa_root: Path) -> str:
    """Identity of the runtime files the SDK builder would actually select."""
    deps = configured_deps_root(root, vespa_root)
    affined_root = (
        vespa_root.parent.parent
        if vespa_root.name == "vespa" and vespa_root.parent.name == "upstream"
        else None
    )

    explicit_extra = os.environ.get("VESPA_EXTRA_LIB_DIR")
    extra_candidates: list[Path] = []
    if explicit_extra:
        extra_candidates.append(Path(explicit_extra))
    if deps is not None:
        extra_candidates.append((deps / "../../usr/lib64").resolve(strict=False))
    if affined_root is not None:
        extra_candidates.append(affined_root / "core/native/sdk/linux-x86_64/lib")
    extra_candidates.append(root / "core/native/sdk/linux-x86_64/lib")

    try:
        worktrees = subprocess.run(
            ["git", "-C", str(root), "worktree", "list", "--porcelain"],
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError:
        worktrees = None
    if worktrees is not None:
        for line in worktrees.stdout.splitlines():
            if line.startswith("worktree "):
                extra_candidates.append(
                    Path(line.removeprefix("worktree "))
                    / "core/native/sdk/linux-x86_64/lib"
                )
    extra_candidates.append(root / "target/native-sdk-package/lib")

    selected_extra = next(
        (directory for directory in extra_candidates if (directory / "libLLVM.so.21.1").is_file()),
        None,
    )

    selected: list[Path] = []
    if selected_extra is not None:
        for name in ("libLLVM.so.21.1", "libxxhash.so"):
            path = selected_extra / name
            if path.is_file():
                selected.append(path)

    explicit_aws = os.environ.get("VESPA_AWS_LIB_DIR")
    ffi_candidates: list[Path] = []
    if explicit_aws:
        ffi_candidates.append(Path(explicit_aws) / "libffi.so.6")
    if selected_extra is not None:
        ffi_candidates.append(selected_extra / "libffi.so.6")
    if deps is not None:
        ffi_candidates.append(
            (deps / "../../usr/local/aws-cli/v2/2.36.43/dist/libffi.so.6").resolve(
                strict=False
            )
        )
    ffi_candidates.append(root / "target/native-sdk-package/lib/libffi.so.6")
    if deps is not None:
        deps_lib = deps / "lib64" if (deps / "lib64").is_dir() else deps / "lib"
        ffi_candidates.append(deps_lib / "libffi.so.6")

    selected_ffi = next((path for path in ffi_candidates if path.is_file()), None)
    if selected_ffi is not None:
        selected.append(selected_ffi)

    digest = hashlib.sha256()
    for path in selected:
        digest.update(path.name.encode())
        digest.update(b"\0")
        try:
            digest.update(file_digest(path.resolve()).encode())
        except OSError as error:
            digest.update(f"error={error}".encode())
        digest.update(b"\n")
    return digest.hexdigest()

def configured_deps_root(root: Path, vespa_root: Path) -> Path | None:
    explicit = os.environ.get("VESPA_DEPS")
    if explicit:
        candidate = Path(explicit)
        return candidate.resolve(strict=False) if candidate.is_dir() else None

    try:
        if vespa_root.name == "vespa" and vespa_root.parent.name == "upstream":
            affined = vespa_root.parent.parent / "vespa-deps"
            if affined.is_dir():
                return affined.resolve()
    except OSError:
        pass

    local = root / "vespa-deps"
    if local.is_dir():
        return local.resolve()

    # Match build-sdk.sh: task worktrees prefer a sibling repository-local
    # dependency installation before falling back to the absolute path recorded
    # by the configured Vespa build.
    try:
        worktrees = subprocess.run(
            ["git", "-C", str(root), "worktree", "list", "--porcelain"],
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError:
        worktrees = None
    if worktrees is not None:
        for line in worktrees.stdout.splitlines():
            if not line.startswith("worktree "):
                continue
            candidate = Path(line.removeprefix("worktree ")) / "vespa-deps"
            if candidate.is_dir():
                return candidate.resolve()

    flags = (
        vespa_root
        / "searchcore/src/tests/proton/documentdb/CMakeFiles/"
        "searchcore_embedded_crud_spike_app.dir/flags.make"
    )
    if flags.is_file():
        text = flags.read_text(encoding="utf-8", errors="replace")
        tokens = text.split()
        for token in tokens:
            if token.endswith("/include"):
                candidate = Path(token).parent
                if candidate.is_dir():
                    return candidate.resolve()
    return None

def dependency_bundle_identity(root: Path, vespa_root: Path) -> str:
    deps = configured_deps_root(root, vespa_root)
    if deps is None:
        return "unresolved"
    lib_dir = Path(os.environ["VESPA_DEPS_LIB_DIR"]) if os.environ.get("VESPA_DEPS_LIB_DIR") else (
        deps / "lib64" if (deps / "lib64").is_dir() else deps / "lib"
    )
    return shared_library_tree_identity(lib_dir)


def vespa_library_identity(vespa_root: Path) -> str:
    """Content identity of the already-built Vespa shared libraries we publish."""
    override = os.environ.get("VESPA_LIBRARY_ROOT")
    if override:
        return shared_library_tree_identity(
            Path(override).resolve(strict=False), "libvespa*.so*"
        )
    return shared_library_tree_identity(vespa_root)


def vespa_config_digest(vespa_root: Path) -> str:
    digest = hashlib.sha256()
    for relative in (
        "searchcore/src/tests/proton/documentdb/CMakeFiles/"
        "searchcore_embedded_crud_spike_app.dir/link.txt",
        "searchcore/src/tests/proton/documentdb/CMakeFiles/"
        "searchcore_embedded_crud_spike_app.dir/flags.make",
    ):
        path = vespa_root / relative
        digest.update(relative.encode())
        digest.update(b"\0")
        if path.is_file():
            digest.update(file_digest(path).encode())
        else:
            digest.update(b"missing")
        digest.update(b"\n")
    return digest.hexdigest()


def rust_identity() -> str:
    return command_output(os.environ.get("RUSTC", "rustc"), "-vV")


def git_tree_identity(path: Path) -> str:
    """Return the Git identity of path without coupling it to a parent worktree HEAD."""
    resolved = path.resolve()
    try:
        top_result = subprocess.run(
            ["git", "-C", str(resolved), "rev-parse", "--show-toplevel"],
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as error:
        return f"error={error}"
    if top_result.returncode != 0:
        return (
            f"status={top_result.returncode}\n"
            f"stdout={top_result.stdout}\n"
            f"stderr={top_result.stderr}"
        )

    top = Path(top_result.stdout.strip()).resolve()
    if top == resolved:
        return command_output("git", "-C", str(resolved), "rev-parse", "HEAD")

    try:
        relative = resolved.relative_to(top).as_posix()
    except ValueError:
        return f"error={resolved} is outside git top-level {top}"
    return command_output("git", "-C", str(top), "rev-parse", f"HEAD:{relative}")


def canonical(fields: dict[str, str]) -> str:
    def escape(value: str) -> str:
        return value.replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t")

    return "".join(f"{key}\t{escape(value)}\n" for key, value in sorted(fields.items()))


def fingerprint(root: Path, vespa_root: Path, target: str, scope: str) -> tuple[str, str]:
    cxx = resolve_program(os.environ.get("CXX", "c++"))
    fields: dict[str, str] = {
        "fingerprint.version": VERSION,
        "fingerprint.scope": scope,
        "build.environment": os.environ.get("SEARCHKERNEL_BUILD_ENV", "host"),
        "cargo.target": target,
        "vespa.commit": git_tree_identity(vespa_root),
        "vespa.config": vespa_config_digest(vespa_root),
        "vespa.library_bundle": vespa_library_identity(vespa_root),
        "native.dependency_bundle": dependency_bundle_identity(root, vespa_root),
        "native.runtime_bundle": runtime_bundle_identity(root, vespa_root),
    }

    if scope == "full":
        fields["flatbuffers.version"] = os.environ.get("FLATBUFFERS_VERSION", "25.12.19")
        fields.update({
            "rustc": rust_identity(),
            "c.compiler": tool_identity("CC", "cc", dumpmachine=True),
            "cxx.compiler": tool_identity("CXX", "c++", dumpmachine=True),
            "ar": tool_identity("AR", "ar"),
            "ranlib": tool_identity("RANLIB", "ranlib"),
            "ld": tool_identity("LD", "ld"),
            "cxx.runtime": (
                command_output(str(cxx), "-print-file-name=libstdc++.so")
                + "\n"
                + command_output(str(cxx), "-print-file-name=libc++.dylib")
            ),
            "libc": (
                command_output("ldd", "--version")
                + "\n"
                + command_output("getconf", "GNU_LIBC_VERSION")
            ),
            "native.source": source_digest(root),
        })
        tracked = (
            "CC", "CXX", "AR", "RANLIB", "LD", "CFLAGS", "CXXFLAGS", "CPPFLAGS",
            "LDFLAGS", "CMAKE", "CMAKE_ARGS", "CMAKE_GENERATOR", "CMAKE_PREFIX_PATH",
            "CMAKE_TOOLCHAIN_FILE", "CXXSTDLIB", "CRATE_CC_NO_DEFAULTS",
        )
        for name in tracked:
            fields[f"env.{name}"] = os.environ.get(name, "")

    if scope == "full":
        defaults = {
            "SK_NATIVE_ENABLE_FAILPOINTS": "0",
            "SK_NATIVE_COVERAGE": "0",
            "SK_NATIVE_SANITIZER": "none",
        }
        for name in (
            "SK_NATIVE_ENABLE_FAILPOINTS",
            "SK_NATIVE_COVERAGE",
            "SK_NATIVE_SANITIZER",
            "SK_NATIVE_EXTRA_CXX_FLAGS",
            "SK_NATIVE_EXTRA_LINK_FLAGS",
        ):
            fields[f"env.{name}"] = os.environ.get(name, defaults.get(name, ""))
    if scope == "full":
        target_underscore = target.replace("-", "_")
        for base in ("CC", "CXX", "AR", "RANLIB", "CFLAGS", "CXXFLAGS", "CPPFLAGS", "LDFLAGS"):
            fields[f"env.{base}_{target}"] = os.environ.get(f"{base}_{target}", "")
            fields[f"env.{base}_{target_underscore}"] = os.environ.get(
                f"{base}_{target_underscore}", ""
            )
            fields[f"env.HOST_{base}"] = os.environ.get(f"HOST_{base}", "")

    text = canonical(fields)
    digest = hashlib.sha256(text.encode()).hexdigest()
    return digest, f"fingerprint-version={VERSION}\nsha256={digest}\n{text}"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--vespa-root", type=Path)
    parser.add_argument("--target", required=True)
    parser.add_argument("--scope", choices=("base", "full"), default="full")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    root = args.root.resolve()
    vespa_root = (args.vespa_root or root / "upstream/vespa").resolve()
    digest, text = fingerprint(root, vespa_root, args.target, args.scope)
    if args.output:
        args.output.write_text(text, encoding="utf-8")
    else:
        print(text, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
