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

ci_runtime_now_epoch() {
    if [[ -n "${SEARCHKERNEL_CI_NOW_EPOCH:-}" ]]; then
        printf '%s\n' "$SEARCHKERNEL_CI_NOW_EPOCH"
    else
        date +%s
    fi
}

ci_runtime_create_volume() {
    local name=$1 kind=$2 lifecycle=$3 created_at
    shift 3
    created_at=$(ci_runtime_now_epoch)

    local -a args=(
        volume create
        --label io.searchkernel.owner=searchkernel
        --label "io.searchkernel.cache-kind=$kind"
        --label "io.searchkernel.lifecycle=$lifecycle"
        --label "io.searchkernel.created-at=$created_at"
    )
    while (($# > 0)); do
        [[ "$1" == *=* ]] || {
            echo "error: invalid Search Kernel Docker volume label: $1" >&2
            return 2
        }
        args+=(--label "$1")
        shift
    done
    args+=("$name")
    docker "${args[@]}"
}

ci_runtime_volume_in_use() {
    local volume=$1
    [[ -n "$(docker ps -aq --filter "volume=$volume" 2>/dev/null || true)" ]]
}

ci_runtime_remove_invocation_containers() {
    local invocation_id=$1 container
    while IFS= read -r container; do
        [[ -n "$container" ]] || continue
        docker rm -f "$container" >/dev/null 2>&1 || true
    done < <(
        docker ps -aq \
            --filter label=io.searchkernel.owner=searchkernel \
            --filter "label=io.searchkernel.invocation-id=$invocation_id" 2>/dev/null || true
    )
}

ci_runtime_age_at_least() {
    local created_at=$1 ttl=$2 now
    [[ "$created_at" =~ ^[0-9]+$ && "$ttl" =~ ^[0-9]+$ ]] || return 1
    now=$(ci_runtime_now_epoch)
    ((now >= created_at && now - created_at >= ttl))
}

ci_runtime_prune_ephemeral_containers() {
    local ttl=${1:-${SEARCHKERNEL_CI_EPHEMERAL_TTL_SECONDS:-86400}}
    local container created_at

    [[ "$ttl" =~ ^[0-9]+$ ]] || {
        echo "error: SEARCHKERNEL_CI_EPHEMERAL_TTL_SECONDS must be an integer" >&2
        return 2
    }

    while IFS= read -r container; do
        [[ -n "$container" ]] || continue
        created_at=$(docker inspect \
            --format '{{index .Config.Labels "io.searchkernel.created-at"}}' \
            "$container" 2>/dev/null || true)
        ci_runtime_age_at_least "$created_at" "$ttl" || continue
        docker rm -f "$container" >/dev/null 2>&1 || true
    done < <(
        docker ps -aq \
            --filter label=io.searchkernel.owner=searchkernel \
            --filter label=io.searchkernel.lifecycle=ephemeral 2>/dev/null || true
    )
}

ci_runtime_prune_ephemeral_volumes() {
    local ttl=${1:-${SEARCHKERNEL_CI_EPHEMERAL_TTL_SECONDS:-86400}}
    local volume created_at

    [[ "$ttl" =~ ^[0-9]+$ ]] || {
        echo "error: SEARCHKERNEL_CI_EPHEMERAL_TTL_SECONDS must be an integer" >&2
        return 2
    }

    while IFS= read -r volume; do
        [[ -n "$volume" ]] || continue
        created_at=$(docker volume inspect \
            --format '{{index .Labels "io.searchkernel.created-at"}}' \
            "$volume" 2>/dev/null || true)
        ci_runtime_age_at_least "$created_at" "$ttl" || continue
        ci_runtime_volume_in_use "$volume" && continue
        docker volume rm -f "$volume" >/dev/null 2>&1 || true
    done < <(
        docker volume ls --quiet \
            --filter label=io.searchkernel.owner=searchkernel \
            --filter label=io.searchkernel.lifecycle=ephemeral 2>/dev/null || true
    )
}

ci_runtime_prune_ephemeral_state() {
    local ttl=${1:-${SEARCHKERNEL_CI_EPHEMERAL_TTL_SECONDS:-86400}}
    # Containers must go first because an abandoned container keeps its volumes
    # referenced and therefore prevents Docker from removing them.
    ci_runtime_prune_ephemeral_containers "$ttl"
    ci_runtime_prune_ephemeral_volumes "$ttl"
}

ci_runtime_created_epoch() {
    local resource=$1 name=$2 created=
    case "$resource" in
        container)
            created=$(docker inspect --format '{{.Created}}' "$name" 2>/dev/null || true)
            ;;
        volume)
            created=$(docker volume inspect --format '{{.CreatedAt}}' "$name" 2>/dev/null || true)
            ;;
        *)
            return 2
            ;;
    esac
    [[ -n "$created" ]] || return 1
    date -d "$created" +%s 2>/dev/null
}

ci_runtime_cache_retirement_ready() {
    local resource=$1 name=$2 ttl=${SEARCHKERNEL_CI_CACHE_RETIRE_TTL_SECONDS:-86400}
    local created_at=

    [[ "$ttl" =~ ^[0-9]+$ ]] || {
        echo "error: SEARCHKERNEL_CI_CACHE_RETIRE_TTL_SECONDS must be an integer" >&2
        return 2
    }
    ((ttl == 0)) && return 0

    case "$resource" in
        container)
            created_at=$(docker inspect \
                --format '{{index .Config.Labels "io.searchkernel.created-at"}}' \
                "$name" 2>/dev/null || true)
            ;;
        volume)
            created_at=$(docker volume inspect \
                --format '{{index .Labels "io.searchkernel.created-at"}}' \
                "$name" 2>/dev/null || true)
            ;;
        *)
            return 2
            ;;
    esac
    if [[ ! "$created_at" =~ ^[0-9]+$ ]]; then
        created_at=$(ci_runtime_created_epoch "$resource" "$name" || true)
    fi
    ci_runtime_age_at_least "$created_at" "$ttl"
}

ci_runtime_prune_legacy_ephemeral_state() {
    local ttl=${1:-${SEARCHKERNEL_CI_LEGACY_TTL_SECONDS:-86400}}
    local container name owner created_at volume cache_kind

    [[ "$ttl" =~ ^[0-9]+$ ]] || {
        echo "error: SEARCHKERNEL_CI_LEGACY_TTL_SECONDS must be an integer" >&2
        return 2
    }

    # One-way migration cleanup for resources created before lifecycle labels
    # existed. The narrow name patterns deliberately exclude shared caches such
    # as sccache and Vespa.
    while IFS= read -r container; do
        [[ -n "$container" ]] || continue
        name=$(docker inspect --format '{{.Name}}' "$container" 2>/dev/null || true)
        name=${name#/}
        case "$name" in
            searchkernel-dev-[0-9]*|searchkernel-ci-[0-9]*) ;;
            *) continue ;;
        esac
        owner=$(docker inspect \
            --format '{{index .Config.Labels "io.searchkernel.owner"}}' \
            "$container" 2>/dev/null || true)
        [[ "$owner" != "searchkernel" ]] || continue
        created_at=$(ci_runtime_created_epoch container "$container" || true)
        ci_runtime_age_at_least "$created_at" "$ttl" || continue
        docker rm -f "$container" >/dev/null 2>&1 || true
    done < <(docker ps -aq 2>/dev/null || true)

    while IFS= read -r volume; do
        [[ -n "$volume" ]] || continue
        case "$volume" in
            searchkernel-dev-src-*|searchkernel-dev-artifacts-*|\
            searchkernel-dev-target-*|\
            searchkernel-ci-src-*|searchkernel-ci-target-*|\
            searchkernel-ci-cargo-registry-*|searchkernel-minimal-ci-*) ;;
            *) continue ;;
        esac
        owner=$(docker volume inspect \
            --format '{{index .Labels "io.searchkernel.owner"}}' \
            "$volume" 2>/dev/null || true)
        [[ "$owner" != "searchkernel" ]] || continue
        if [[ "$volume" == searchkernel-dev-target-* ]]; then
            cache_kind=$(docker volume inspect \
                --format '{{index .Labels "io.searchkernel.cache-kind"}}' \
                "$volume" 2>/dev/null || true)
            [[ -z "$cache_kind" || "$cache_kind" == "<no value>" ]] || continue
        fi
        created_at=$(ci_runtime_created_epoch volume "$volume" || true)
        ci_runtime_age_at_least "$created_at" "$ttl" || continue
        ci_runtime_volume_in_use "$volume" && continue
        docker volume rm -f "$volume" >/dev/null 2>&1 || true
    done < <(docker volume ls --quiet 2>/dev/null || true)
}

ci_runtime_target_volume() {
    local root=$1 environment_id=$2 source_head=$3 canonical
    [[ -n "$environment_id" && "$environment_id" != "<no value>" ]] || {
        echo "error: canonical leaf is missing environment identity" >&2
        return 1
    }
    [[ "$source_head" =~ ^[0-9a-fA-F]{40}$ || "$source_head" =~ ^[0-9a-fA-F]{64}$ ]] || {
        echo "error: target cache requires an immutable source HEAD" >&2
        return 1
    }
    canonical=$(cd -- "$root" && pwd -P)
    python3 - "$canonical" "$environment_id" "${source_head,,}" <<'PY'
import hashlib, os, sys
root, environment_id, source_head = sys.argv[1:]
identity = hashlib.sha256(
    (root + "\0" + environment_id + "\0" + source_head).encode()
).hexdigest()[:32]
print(f"searchkernel-dev-target-{os.getuid()}-{identity}")
PY
}

ci_runtime_prune_target_volumes() {
    local root=$1 keep=$2 volume worktree
    while IFS= read -r volume; do
        [[ -n "$volume" && "$volume" != "$keep" ]] || continue
        worktree=$(docker volume inspect --format '{{index .Labels "io.searchkernel.worktree"}}' "$volume" 2>/dev/null || true)
        if [[ "$worktree" == "$root" || ( -n "$worktree" && ! -d "$worktree" ) ]]; then
            ci_runtime_volume_in_use "$volume" && continue
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

ci_runtime_prune_shared_download_volumes() {
    local environment_id=$1 keep=$2 volume volume_environment
    while IFS= read -r volume; do
        [[ -n "$volume" && "$volume" != "$keep" ]] || continue
        volume_environment=$(docker volume inspect \
            --format '{{index .Labels "io.searchkernel.environment-id"}}' \
            "$volume" 2>/dev/null || true)
        [[ -n "$volume_environment" && "$volume_environment" != "$environment_id" ]] || continue
        ci_runtime_cache_retirement_ready volume "$volume" || continue
        ci_runtime_volume_in_use "$volume" && continue
        docker volume rm -f "$volume" >/dev/null 2>&1 || true
    done < <(
        docker volume ls --quiet \
            --filter label=io.searchkernel.cache-kind=immutable-download 2>/dev/null || true
    )
}

ci_runtime_prune_sccache_generations() {
    local current_binary=$1 current_socket=$2 current_cache
    local container socket consumer busy volume
    current_cache=${current_socket/searchkernel-ci-sccache-socket-/searchkernel-ci-sccache-cache-}

    [[ "$current_binary" == searchkernel-ci-sccache-bin-* ]] || return 2
    [[ "$current_socket" == searchkernel-ci-sccache-socket-* ]] || return 2
    [[ "$current_cache" == searchkernel-ci-sccache-cache-* ]] || return 2

    # Servers are persistent by design, but only the server using the selected
    # socket generation is current. Inspect every versioned sccache server so a
    # future sccache upgrade and a toolchain rotation use the same retirement path.
    while IFS= read -r container; do
        [[ -n "$container" ]] || continue
        socket=$(docker inspect \
            --format '{{range .Mounts}}{{if eq .Destination "/sccache"}}{{.Name}}{{end}}{{end}}' \
            "$container" 2>/dev/null || true)
        [[ "$socket" != "$current_socket" ]] || continue
        ci_runtime_cache_retirement_ready container "$container" || continue

        busy=0
        if [[ -n "$socket" ]]; then
            while IFS= read -r consumer; do
                [[ -n "$consumer" ]] || continue
                [[ "$consumer" == "$container" ]] || {
                    busy=1
                    break
                }
            done < <(docker ps -aq --filter "volume=$socket" 2>/dev/null || true)
        fi
        ((busy == 0)) || continue
        docker rm -f "$container" >/dev/null 2>&1 || true
    done < <(
        docker ps -aq \
            --filter label=io.searchkernel.sccache-version 2>/dev/null || true
    )

    # Once old servers are gone, any non-current sccache volume is an obsolete
    # generation. This also migrates pre-label and pre-toolchain-partition names.
    while IFS= read -r volume; do
        [[ -n "$volume" ]] || continue
        case "$volume" in
            searchkernel-ci-sccache-*) ;;
            *) continue ;;
        esac
        case "$volume" in
            "$current_binary"|"$current_cache"|"$current_socket") continue ;;
        esac
        ci_runtime_cache_retirement_ready volume "$volume" || continue
        ci_runtime_volume_in_use "$volume" && continue
        docker volume rm -f "$volume" >/dev/null 2>&1 || true
    done < <(docker volume ls --quiet 2>/dev/null || true)
}

ci_runtime_prune_vespa_volumes() {
    local keep=$1 volume
    while IFS= read -r volume; do
        [[ -n "$volume" && "$volume" != "$keep" ]] || continue
        case "$volume" in
            searchkernel-ci-vespa-*) ;;
            *) continue ;;
        esac
        ci_runtime_cache_retirement_ready volume "$volume" || continue
        ci_runtime_volume_in_use "$volume" && continue
        docker volume rm -f "$volume" >/dev/null 2>&1 || true
    done < <(docker volume ls --quiet 2>/dev/null || true)
}
