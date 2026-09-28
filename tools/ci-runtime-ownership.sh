#!/usr/bin/env bash
# Runtime identity and writable-cache ownership helpers for canonical ./dev.

ci_runtime_require_digest() {
    local ref=${1:-} owner=${2:-image}
    [[ -n "$ref" ]] || {
        echo "error: $owner is missing immutable Vespa image digest" >&2
        return 1
    }
    [[ "$ref" == *@sha256:* ]] || {
        echo "error: $owner Vespa image identity is not immutable: $ref" >&2
        return 1
    }
    printf '%s\n' "$ref"
}

ci_runtime_target_volume() {
    local root=$1 environment_id=$2 canonical
    [[ -n "$environment_id" && "$environment_id" != "<no value>" ]] || {
        echo "error: canonical leaf is missing environment identity" >&2
        return 1
    }
    canonical=$(cd -- "$root" && pwd -P)
    python3 - "$canonical" "$environment_id" <<'PY'
import hashlib, os, sys
root, environment_id = sys.argv[1:]
identity = hashlib.sha256((root + "\0" + environment_id).encode()).hexdigest()[:32]
print(f"searchkernel-dev-target-{os.getuid()}-{identity}")
PY
}

ci_runtime_prune_target_volumes() {
    local root=$1 keep=$2 volume worktree
    while IFS= read -r volume; do
        [[ -n "$volume" && "$volume" != "$keep" ]] || continue
        worktree=$(docker volume inspect --format '{{index .Labels "io.searchkernel.worktree"}}' "$volume" 2>/dev/null || true)
        if [[ "$worktree" == "$root" || ( -n "$worktree" && ! -d "$worktree" ) ]]; then
            docker volume rm -f "$volume" >/dev/null 2>&1 || true
        fi
    done < <(docker volume ls --quiet --filter label=io.searchkernel.cache-kind=worktree-target)
}

ci_runtime_shared_download_volume() {
    local kind=$1 environment_id=$2 identity
    [[ "$kind" =~ ^[a-z0-9-]+$ ]] || return 2
    [[ -n "$environment_id" && "$environment_id" != "<no value>" ]] || return 2
    identity=$(printf '%s' "$environment_id" | sha256sum | cut -c1-24)
    printf 'searchkernel-dev-download-%s-%s-%s\n' "$(id -u)" "$kind" "$identity"
}
