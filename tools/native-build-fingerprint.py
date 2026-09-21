#!/usr/bin/env python3
"""Create a deterministic fingerprint for Search Kernel native artifacts."""

from __future__ import annotations

import argparse
import hashlib
import os
import subprocess
from pathlib import Path


VERSION = "2"


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


def canonical(fields: dict[str, str]) -> str:
    def escape(value: str) -> str:
        return value.replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t")

    return "".join(f"{key}\t{escape(value)}\n" for key, value in sorted(fields.items()))


def fingerprint(root: Path, vespa_root: Path, target: str) -> tuple[str, str]:
    cxx = resolve_program(os.environ.get("CXX", "c++"))
    fields: dict[str, str] = {
        "fingerprint.version": VERSION,
        "build.environment": os.environ.get("SEARCHKERNEL_BUILD_ENV", "host"),
        "cargo.target": target,
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
        "flatbuffers.version": os.environ.get("FLATBUFFERS_VERSION", "25.12.19"),
        "vespa.commit": command_output("git", "-C", str(vespa_root), "rev-parse", "HEAD"),
        "vespa.config": vespa_config_digest(vespa_root),
        "native.source": source_digest(root),
    }

    tracked = (
        "CC", "CXX", "AR", "RANLIB", "LD", "CFLAGS", "CXXFLAGS", "CPPFLAGS",
        "LDFLAGS", "CMAKE", "CMAKE_ARGS", "CMAKE_GENERATOR", "CMAKE_PREFIX_PATH",
        "CMAKE_TOOLCHAIN_FILE", "CXXSTDLIB",
        "CRATE_CC_NO_DEFAULTS", "SK_NATIVE_ENABLE_FAILPOINTS", "SK_NATIVE_COVERAGE",
        "SK_NATIVE_SANITIZER", "SK_NATIVE_EXTRA_CXX_FLAGS", "SK_NATIVE_EXTRA_LINK_FLAGS",
    )
    for name in tracked:
        defaults = {
            "SK_NATIVE_ENABLE_FAILPOINTS": "0",
            "SK_NATIVE_COVERAGE": "0",
            "SK_NATIVE_SANITIZER": "none",
        }
        fields[f"env.{name}"] = os.environ.get(name, defaults.get(name, ""))
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
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    root = args.root.resolve()
    vespa_root = (args.vespa_root or root / "upstream/vespa").resolve()
    digest, text = fingerprint(root, vespa_root, args.target)
    if args.output:
        args.output.write_text(text, encoding="utf-8")
    else:
        print(text, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
