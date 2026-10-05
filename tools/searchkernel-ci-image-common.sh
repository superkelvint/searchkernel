#!/usr/bin/env bash

# Shared helpers for publishing Search Kernel's split CI-image artifacts.
# Callers are expected to set -euo pipefail before sourcing this file.

ci_image_die() {
    echo "ERROR: $*" >&2
    exit 1
}

ci_image_require_commands() {
    local cmd
    for cmd in "$@"; do
        command -v "$cmd" >/dev/null 2>&1 ||
            ci_image_die "missing required command: $cmd"
    done
}

ci_image_require_docker() {
    ci_image_require_commands docker
    docker info >/dev/null 2>&1 ||
        ci_image_die "Docker is not usable by the current user"
}

ci_image_require_clean_checkout() {
    local root=$1
    local status

    status="$(git -C "$root" status --porcelain --untracked-files=normal)"
    [[ -z "$status" ]] ||
        ci_image_die "publisher checkout has uncommitted or untracked files; publish only from a clean checkout"
}

ci_image_require_main_checkout() {
    local root=$1
    local branch

    branch="$(git -C "$root" branch --show-current)"
    [[ "$branch" == "main" ]] ||
        ci_image_die "publishing :latest requires the main branch; current branch is ${branch:-detached HEAD}"
}

ci_image_login() {
    if [[ -n "${DOCKERHUB_TOKEN:-}" ]]; then
        local username=${DOCKERHUB_USERNAME:-superkelvint}
        printf '%s' "$DOCKERHUB_TOKEN" |
            docker login --username "$username" --password-stdin
    else
        echo "No DOCKERHUB_TOKEN supplied; using existing Docker CLI credentials."
        echo "If push authentication fails, run: docker login"
    fi
}

ci_image_resolve_local_digest() {
    local image=$1
    local digests_json

    digests_json="$(docker image inspect --format '{{json .RepoDigests}}' "$image")" ||
        ci_image_die "could not inspect pulled image: $image"

    python3 - "$image" "$digests_json" <<'PY'
import json
import sys

image, encoded = sys.argv[1:]
digests = json.loads(encoded)
if not digests:
    raise SystemExit(f"pulled image has no repository digest: {image}")

def repository(ref):
    ref = ref.split("@", 1)[0]
    slash = ref.rfind("/")
    colon = ref.rfind(":")
    if colon > slash:
        ref = ref[:colon]
    for prefix in ("docker.io/", "index.docker.io/"):
        if ref.startswith(prefix):
            ref = ref[len(prefix):]
    if "/" not in ref:
        ref = "library/" + ref
    return ref

wanted = repository(image)
matches = {
    digest
    for item in digests
    if "@" in item
    for candidate, digest in [item.rsplit("@", 1)]
    if repository(candidate) == wanted
}

if len(matches) != 1:
    raise SystemExit(
        f"expected exactly one content digest for repository {wanted!r}; "
        f"found {sorted(matches)!r} in {digests!r}"
    )

digest = next(iter(matches))
requested = image.split("@", 1)[0]
slash = requested.rfind("/")
colon = requested.rfind(":")
if colon > slash:
    requested = requested[:colon]
print(f"{requested}@{digest}")
PY
}

ci_image_pull_and_resolve_digest() {
    local image=$1

    docker pull "$image" >/dev/null ||
        ci_image_die "could not pull image: $image"
    ci_image_resolve_local_digest "$image"
}

ci_image_toolchain_id() {
    local root=$1
    local base_image_digest=$2
    python3 - "$root" "$base_image_digest" <<'PY'
import hashlib
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
base_image_digest = sys.argv[2]
if not root.is_dir():
    raise SystemExit(f"repository root does not exist: {root}")
if "@sha256:" not in base_image_digest:
    raise SystemExit(
        "toolchain identity requires an immutable base-image digest, "
        f"found {base_image_digest!r}"
    )

recipe = root / "docker/searchkernel-ci-toolchain"
inputs = [
    *sorted(path for path in recipe.rglob("*") if path.is_file()),
    root / "clients/typescript/package.json",
    root / "clients/typescript/package-lock.json",
]
for path in inputs:
    if not path.is_file():
        raise SystemExit(f"toolchain identity input does not exist: {path}")

h = hashlib.sha256()
h.update(b"searchkernel-ci-toolchain-v3\0")
h.update(base_image_digest.encode())
h.update(b"\0")
for path in inputs:
    relative = path.relative_to(root).as_posix().encode()
    data = path.read_bytes()
    h.update(relative)
    h.update(b"\0")
    h.update(len(data).to_bytes(8, "big"))
    h.update(data)

print(h.hexdigest())
PY
}

ci_image_vespa_bundle_id() {
    local root=$1
    python3 - "$root" <<'PY'
import hashlib
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
inputs = [
    root / "docker/searchkernel-ci-vespa",
    root / "tools/prepare-searchkernel-ci-native-context.sh",
    root / "tools/bundle-native-runtime-closure.py",
    root / "tools/check-native-runtime-closure.py",
]

files = []
for item in inputs:
    if item.is_dir():
        files.extend(path for path in item.rglob("*") if path.is_file())
    elif item.is_file():
        files.append(item)
    else:
        raise SystemExit(f"Vespa bundle recipe input does not exist: {item}")

h = hashlib.sha256()
h.update(b"searchkernel-ci-vespa-bundle-v1\0")
for path in sorted(files):
    relative = path.relative_to(root).as_posix().encode()
    data = path.read_bytes()
    h.update(relative)
    h.update(b"\0")
    h.update(len(data).to_bytes(8, "big"))
    h.update(data)

print(h.hexdigest())
PY
}

ci_image_vespa_producer_id() {
    local root=$1
    local full_sdk=$2
    python3 - "$root" "$full_sdk" <<'PY'
import hashlib
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
full_sdk = pathlib.Path(sys.argv[2])
inputs = [
    root / "core/native/vespa/manifest.json",
    full_sdk / "manifest.json",
    full_sdk / "native-build-fingerprint-v2.txt",
]

h = hashlib.sha256()
h.update(b"searchkernel-ci-vespa-producer-v1\0")
for path in inputs:
    if not path.is_file():
        raise SystemExit(f"Vespa producer identity input does not exist: {path}")
    data = path.read_bytes()
    h.update(path.name.encode())
    h.update(b"\0")
    h.update(len(data).to_bytes(8, "big"))
    h.update(data)

print(h.hexdigest())
PY
}

ci_image_environment_id() {
    local recipe=$1
    local toolchain_image_digest=$2
    local vespa_image_digest=$3
    python3 - "$recipe" "$toolchain_image_digest" "$vespa_image_digest" <<'PY'
import hashlib
import pathlib
import sys

recipe = pathlib.Path(sys.argv[1])
toolchain_image_digest = sys.argv[2]
vespa_image_digest = sys.argv[3]
if not recipe.is_file():
    raise SystemExit(f"CI environment recipe does not exist: {recipe}")
for label, value in (
    ("toolchain", toolchain_image_digest),
    ("Vespa", vespa_image_digest),
):
    if "@sha256:" not in value:
        raise SystemExit(
            f"CI environment identity requires immutable {label} image digest, found {value!r}"
        )

h = hashlib.sha256()
h.update(b"searchkernel-ci-environment-v1\0")
h.update(toolchain_image_digest.encode())
h.update(b"\0")
h.update(vespa_image_digest.encode())
h.update(b"\0")
h.update(recipe.read_bytes())
print(h.hexdigest())
PY
}

ci_image_vespa_commit() {
    local root=$1
    python3 - "$root/core/native/vespa/manifest.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    value = json.load(stream)["vespa_commit"]

if len(value) != 40 or any(ch not in "0123456789abcdef" for ch in value):
    raise SystemExit(f"invalid pinned Vespa commit: {value!r}")
print(value)
PY
}

ci_image_label() {
    local image=$1
    local label=$2
    docker image inspect --format "{{index .Config.Labels \"${label}\"}}" "$image"
}

ci_image_require_label() {
    local image=$1
    local label=$2
    local expected=$3
    local actual

    actual="$(ci_image_label "$image" "$label")"
    if [[ "$actual" != "$expected" ]]; then
        ci_image_die "$image label $label mismatch: expected $expected, found ${actual:-<missing>}"
    fi
}

ci_image_pull_if_available() {
    local image=$1
    docker pull "$image" >/dev/null 2>&1
}
