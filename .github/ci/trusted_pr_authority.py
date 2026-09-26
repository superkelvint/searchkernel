#!/usr/bin/env python3
"""Classify changes that can redefine Search Kernel merge authorization.

This module is executed from trusted default-branch state.  It deliberately
separates verification *authority* (workflows, selectors, command wrappers,
verifiers) from ordinary candidate implementation.  A candidate that changes
authority cannot authorize itself through the normal trusted gate.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections.abc import Iterable
from pathlib import Path, PurePosixPath

PROTECTED_PREFIXES = (
    ".github/ci/",
    ".github/workflows/",
    "verification/",
    "reference/vespa/",
    "tools/",
    ".moon/",
)
PROTECTED_FILES = {"dev", "moon.yml", "workspace.yml", ".prototools"}
TRANSITION_COMPANION_PREFIXES = ("docs/",)
TRANSITION_COMPANION_FILES = {"README.md", "BUILDING.md", "AGENTS.md"}

# Some candidate manifests legitimately change dependencies or product metadata,
# but selected keys are themselves command mappings.  Compare only those keys so
# a dependency bump does not become an authority transition while `npm test=true`
# does.
SEMANTIC_AUTHORITY = {
    "clients/typescript/package.json": ("scripts",),
}


def normalize(path: str) -> str:
    value = PurePosixPath(path.strip().replace("\\", "/")).as_posix()
    return value[2:] if value.startswith("./") else value


def protected(path: str) -> bool:
    path = normalize(path)
    return path in PROTECTED_FILES or any(path.startswith(prefix) for prefix in PROTECTED_PREFIXES)


def transition_companion(path: str) -> bool:
    path = normalize(path)
    return path in TRANSITION_COMPANION_FILES or any(
        path.startswith(prefix) for prefix in TRANSITION_COMPANION_PREFIXES
    )


def _git_show(repo: Path, revision: str, path: str) -> str | None:
    result = subprocess.run(
        ["git", "-C", str(repo), "show", f"{revision}:{path}"],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    return result.stdout if result.returncode == 0 else None


def _json_key(repo: Path, revision: str, path: str, key: str):
    content = _git_show(repo, revision, path)
    if content is None:
        return None
    try:
        document = json.loads(content)
    except json.JSONDecodeError:
        # Malformed candidate/base authority is itself an authority change and
        # must fail closed instead of making classification silently permissive.
        return object()
    return document.get(key)


def semantic_authority_changes(
    changed_paths: Iterable[str], base: str | None, head: str | None, repo: Path
) -> list[str]:
    if not base or not head:
        return []
    changed = {normalize(path) for path in changed_paths if path.strip()}
    findings: list[str] = []
    for path, keys in SEMANTIC_AUTHORITY.items():
        if path not in changed:
            continue
        for key in keys:
            before = _json_key(repo, base, path, key)
            after = _json_key(repo, head, path, key)
            if before != after:
                findings.append(f"{path}#{key}")
    return findings


def classify(
    paths: Iterable[str], *, base: str | None = None, head: str | None = None, repo: Path = Path(".")
) -> tuple[list[str], list[str], bool]:
    normalized = [normalize(path) for path in paths if path.strip()]
    direct = sorted(path for path in normalized if protected(path))
    semantic = sorted(semantic_authority_changes(normalized, base, head, repo))
    semantic_paths = {item.split("#", 1)[0] for item in semantic}
    transition_only = bool(direct or semantic) and all(
        protected(path) or path in semantic_paths or transition_companion(path)
        for path in normalized
    )
    return direct, semantic, transition_only


def write_output(path: Path, *, authority_changed: bool, transition_only: bool) -> None:
    with path.open("a", encoding="utf-8") as stream:
        stream.write(f"authority_changed={'true' if authority_changed else 'false'}\n")
        stream.write(f"transition_only={'true' if transition_only else 'false'}\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--changed-paths", required=True, type=Path)
    parser.add_argument("--base")
    parser.add_argument("--head")
    parser.add_argument("--repo", type=Path, default=Path("."))
    parser.add_argument("--github-output", type=Path)
    parser.add_argument("--require-transition-only", action="store_true")
    args = parser.parse_args()

    paths = [line.strip() for line in args.changed_paths.read_text(encoding="utf-8").splitlines() if line.strip()]
    direct, semantic, transition_only = classify(
        paths, base=args.base, head=args.head, repo=args.repo
    )
    authority_changed = bool(direct or semantic)

    if args.github_output is not None:
        write_output(
            args.github_output,
            authority_changed=authority_changed,
            transition_only=transition_only,
        )

    if authority_changed:
        print("Trusted verification authority changes:")
        for item in direct:
            print(f"  {item}")
        for item in semantic:
            print(f"  {item}")
    else:
        print("Trusted verification authority is unchanged by the candidate.")

    if args.require_transition_only and (not authority_changed or not transition_only):
        if not authority_changed:
            print("transition denied: candidate does not change verification authority", file=sys.stderr)
        else:
            print(
                "transition denied: split product changes from verification-authority changes",
                file=sys.stderr,
            )
        return 3
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
