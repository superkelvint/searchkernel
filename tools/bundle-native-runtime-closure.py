#!/usr/bin/env python3
"""Bundle non-baseline ELF DT_NEEDED closure from the trusted producer host."""

from __future__ import annotations

import argparse
import pathlib
import json
import shutil
import sys

from native_runtime_closure import (
    SYSTEM_BASELINE,
    is_elf,
    elf_soname,
    ldd_resolutions,
    needed_libraries,
    resolve_origin_dependency,
    runtime_candidates,
    sha256_file,
)


def die(message: str) -> "NoReturn":
    raise SystemExit(message)


def bundle_entries(libdir: pathlib.Path) -> list[pathlib.Path]:
    root = libdir.resolve()
    payloads: set[pathlib.Path] = set()

    for entry in libdir.iterdir():
        try:
            resolved = entry.resolve(strict=True)
        except OSError as exc:
            die(f"broken native runtime entry {entry}: {exc}")
        try:
            resolved.relative_to(root)
        except ValueError:
            die(f"native runtime entry escapes bundled lib directory: {entry} -> {resolved}")
        if not resolved.is_file() or not is_elf(resolved):
            continue
        payloads.add(resolved)

    if not payloads:
        die(f"native runtime lib directory contains no ELF shared libraries: {libdir}")
    return sorted(payloads, key=str)


def source_identity(path: pathlib.Path, source_roots: list[tuple[str, pathlib.Path]]) -> str:
    resolved = path.resolve(strict=True)
    for label, root in source_roots:
        try:
            return f"{label}/{resolved.relative_to(root)}"
        except ValueError:
            continue
    return f"external/{resolved.name}/{sha256_file(resolved)}"


def bundle(
    libdir: pathlib.Path,
    search_dirs: list[pathlib.Path],
    extra_elfs: list[pathlib.Path],
    source_roots: list[tuple[str, pathlib.Path]],
) -> int:
    if not libdir.is_dir():
        die(f"native runtime lib directory does not exist: {libdir}")

    readelf = shutil.which("readelf")
    ldd = shutil.which("ldd")
    if readelf is None or ldd is None:
        die("native runtime bundling requires readelf and ldd")

    root = libdir.resolve()
    resolved_search_dirs: list[pathlib.Path] = []
    for directory in search_dirs:
        try:
            resolved = directory.resolve(strict=True)
        except OSError as exc:
            die(f"runtime search directory is unavailable: {directory}: {exc}")
        if not resolved.is_dir():
            die(f"runtime search path is not a directory: {resolved}")
        resolved_search_dirs.append(resolved)

    queue = bundle_entries(libdir)
    bundled_sonames: dict[str, tuple[str, pathlib.Path]] = {}
    for payload in queue:
        try:
            needed_name = elf_soname(payload, readelf)
        except RuntimeError as exc:
            die(str(exc))
        if needed_name:
            digest = sha256_file(payload)
            previous = bundled_sonames.get(needed_name)
            if previous is not None and previous[0] != digest:
                die(
                    f"conflicting bundled ELF candidates provide SONAME {needed_name}: "
                    f"{previous[1]} vs {payload}"
                )
            bundled_sonames[needed_name] = (digest, payload)
    trusted_source_roots = [root, *resolved_search_dirs, *(source_root for _, source_root in source_roots)]
    for extra in extra_elfs:
        try:
            resolved_extra = extra.resolve(strict=True)
        except OSError as exc:
            die(f"invalid extra native ELF {extra}: {exc}")
        if not resolved_extra.is_file() or not is_elf(resolved_extra):
            die(f"extra native runtime input is not an ELF file: {resolved_extra}")
        queue.append(resolved_extra)

    inspected: set[pathlib.Path] = set()
    selected: dict[str, tuple[str, pathlib.Path]] = {}
    required_by: dict[pathlib.Path, set[str]] = {}
    provenance: dict[pathlib.Path, dict[str, str]] = {}
    provenance_path = libdir / ".searchkernel-runtime-provenance.json"
    prior_identities: dict[tuple[str, str, str | None], str] = {}
    if provenance_path.exists():
        try:
            previous = json.loads(provenance_path.read_text(encoding="utf-8"))
            for item in previous.get("libraries", []):
                if isinstance(item, dict):
                    prior_identities[(item.get("packaged_name"), item.get("sha256"), item.get("soname"))] = item.get("source_identity")
        except (OSError, json.JSONDecodeError) as exc:
            die(f"invalid existing runtime provenance {provenance_path}: {exc}")
    for payload in queue:
        resolved = payload.resolve(strict=True)
        provenance[resolved] = {
            "source_identity": source_identity(resolved, source_roots),
            "sha256": sha256_file(resolved),
        }
    copied = 0

    while queue:
        payload = queue.pop(0)
        resolved_payload = payload.resolve(strict=True)
        if resolved_payload in inspected:
            continue
        inspected.add(resolved_payload)

        try:
            needed_names = needed_libraries(resolved_payload, readelf)
        except RuntimeError as exc:
            die(str(exc))
        try:
            resolutions = ldd_resolutions(
                resolved_payload,
                ldd,
                [*resolved_search_dirs, root],
                trusted_source_roots,
            )
        except RuntimeError as exc:
            die(str(exc))

        for needed in sorted(needed_names):
            if needed in SYSTEM_BASELINE:
                continue

            if needed.startswith(("$ORIGIN/", "${ORIGIN}/")):
                origin_dependency = resolve_origin_dependency(resolved_payload, needed)
                if origin_dependency is None:
                    die(
                        f"{resolved_payload.name}: unresolved origin-relative "
                        f"runtime dependency {needed}"
                    )
                allowed_origin_roots = [root]
                if not resolved_payload.is_relative_to(root):
                    allowed_origin_roots.append(resolved_payload.parent)
                resolved_origin = origin_dependency.resolve(strict=True)
                if not any(
                    resolved_origin.is_relative_to(origin_root)
                    for origin_root in allowed_origin_roots
                ):
                    die(
                        f"{resolved_payload.name}: origin-relative dependency escapes "
                        f"its packaged runtime directory: {needed}"
                    )
                queue.append(origin_dependency)
                required_by.setdefault(origin_dependency.resolve(strict=True), set()).add(
                    str(resolved_payload.relative_to(root))
                    if resolved_payload.is_relative_to(root)
                    else resolved_payload.name
                )
                continue
            candidates = runtime_candidates(
                needed,
                [*resolved_search_dirs, root],
                trusted_source_roots,
            )
            candidate_hashes = {sha256_file(candidate) for candidate in candidates}
            if len(candidate_hashes) > 1:
                die(
                    f"conflicting producer candidates for {needed}: "
                    + ", ".join(str(candidate) for candidate in candidates)
                )
            source = resolutions.get(needed)
            if source is None:
                die(
                    f"{resolved_payload.name}: could not resolve non-baseline "
                    f"runtime dependency {needed} from the trusted producer environment"
                )
            try:
                source = source.resolve(strict=True)
            except OSError as exc:
                die(f"{resolved_payload.name}: invalid runtime dependency {needed}: {exc}")
            if not source.is_file() or not is_elf(source):
                die(
                    f"{resolved_payload.name}: resolved runtime dependency "
                    f"{needed} is not an ELF file: {source}"
                )

            try:
                source.relative_to(root)
                source_is_bundled = True
            except ValueError:
                source_is_bundled = False
            if source_is_bundled:
                required_by.setdefault(source, set()).add(
                    str(resolved_payload.relative_to(root))
                    if resolved_payload.is_relative_to(root)
                    else resolved_payload.name
                )
                queue.append(source)
                continue

            if not any(source.is_relative_to(source_root) for source_root in trusted_source_roots):
                die(
                    f"{resolved_payload.name}: runtime dependency {needed} resolved outside declared producer roots: {source}"
                )
            digest = sha256_file(source)
            previous = selected.get(needed)
            if previous is not None and previous[0] != digest:
                die(
                    f"conflicting producer resolution for {needed}: "
                    f"{previous[1]} vs {source}"
                )
            selected[needed] = (digest, source)
            destination = root / needed
            if destination.is_symlink():
                destination.unlink()
            if destination.exists():
                destination.unlink()
            shutil.copy2(source, destination)
            queue.append(destination)
            resolved_destination = destination.resolve(strict=True)
            provenance[resolved_destination] = {
                "source_identity": source_identity(source, source_roots),
                "sha256": digest,
            }
            required_by.setdefault(resolved_destination, set()).add(
                str(resolved_payload.relative_to(root))
                if resolved_payload.is_relative_to(root)
                else resolved_payload.name
            )
            copied += 1
            print(f"bundled runtime dependency {needed} from {source}", file=sys.stderr)

    entries = []
    seen_payloads: set[pathlib.Path] = set()
    for entry in sorted(libdir.iterdir(), key=lambda item: item.name):
        resolved = entry.resolve(strict=True)
        if resolved in seen_payloads or not resolved.is_file() or not is_elf(resolved):
            continue
        seen_payloads.add(resolved)
        entry_soname = elf_soname(resolved, readelf)
        preserved_identity = prior_identities.get((entry.name, sha256_file(resolved), entry_soname))
        identity = provenance.get(resolved, {
            "source_identity": source_identity(resolved, source_roots),
            "sha256": sha256_file(resolved),
        })
        try:
            soname = elf_soname(resolved, readelf)
        except RuntimeError as exc:
            die(str(exc))
        entries.append({
            "packaged_name": entry.name,
            "soname": soname,
            "sha256": identity["sha256"],
            "source_identity": preserved_identity or identity["source_identity"],
            "required_by": sorted(required_by.get(resolved, set())),
        })
    provenance_path = libdir / ".searchkernel-runtime-provenance.json"
    provenance_path.write_text(
        json.dumps({"format_version": 1, "libraries": entries}, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    return copied


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("libdir", type=pathlib.Path)
    parser.add_argument(
        "--search-dir",
        action="append",
        default=[],
        type=pathlib.Path,
        help="additional trusted producer library directory used for ldd resolution",
    )
    parser.add_argument(
        "--extra-elf",
        action="append",
        default=[],
        type=pathlib.Path,
        help="ELF outside libdir whose DT_NEEDED dependencies must also be bundled",
    )
    parser.add_argument(
        "--source-root",
        action="append",
        default=[],
        metavar="LABEL=PATH",
        help="named producer root used to create relocatable provenance identities",
    )
    args = parser.parse_args()
    source_roots: list[tuple[str, pathlib.Path]] = []
    for value in args.source_root:
        label, separator, raw_path = value.partition("=")
        if not separator or not label or not raw_path:
            die(f"invalid source root {value!r}; expected LABEL=PATH")
        source_roots.append((label, pathlib.Path(raw_path).resolve(strict=True)))
    count = bundle(args.libdir, args.search_dir, args.extra_elf, source_roots)
    print(f"bundled {count} non-baseline runtime dependencies", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
