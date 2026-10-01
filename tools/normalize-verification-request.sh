#!/usr/bin/env bash
set -euo pipefail

event_name=${EVENT_NAME:-}

case "$event_name" in
    repository_dispatch)
        revision=${DISPATCH_REVISION:-}
        operation=${DISPATCH_OPERATION:-}
        target=${DISPATCH_TARGET:-}
        request_id=${DISPATCH_REQUEST_ID:-}
        ;;
    issue_comment)
        if [[ -z "${PULL_REQUEST_URL:-}" ]]; then
            echo 'error: comment verification requests must be made on a pull request' >&2
            exit 2
        fi
        case "${COMMENT_ASSOCIATION:-}" in
            OWNER|MEMBER|COLLABORATOR) ;;
            *)
                echo 'error: comment author is not authorized to request verification' >&2
                exit 2
                ;;
        esac
        if [[ ! "${COMMENT_ID:-}" =~ ^[0-9]+$ ]]; then
            echo 'error: comment id must be numeric' >&2
            exit 2
        fi

        mapfile -t comment_lines <<< "${COMMENT_BODY:-}"
        if [[ "${#comment_lines[@]}" -ne 1 ]]; then
            echo 'error: verification comment must be exactly one line' >&2
            exit 2
        fi
        read -r -a fields <<< "${comment_lines[0]}"
        if [[ "${fields[0]:-}" != '/searchkernel-verify' ]]; then
            echo 'error: verification comment must begin with /searchkernel-verify' >&2
            exit 2
        fi

        operation=${fields[1]:-}
        case "$operation" in
            ci)
                if [[ "${#fields[@]}" -ne 3 ]]; then
                    echo 'error: usage: /searchkernel-verify ci <exact-sha>' >&2
                    exit 2
                fi
                target=
                revision=${fields[2]}
                ;;
            test|verify)
                if [[ "${#fields[@]}" -ne 4 ]]; then
                    echo 'error: usage: /searchkernel-verify {test|verify} <target> <exact-sha>' >&2
                    exit 2
                fi
                target=${fields[2]}
                revision=${fields[3]}
                ;;
            *)
                echo 'error: comment operation must be ci, test, or verify' >&2
                exit 2
                ;;
        esac
        request_id="comment-${COMMENT_ID}"
        ;;
    *)
        echo "error: unsupported verification event: ${event_name}" >&2
        exit 2
        ;;
esac

if [[ ! "$revision" =~ ^[0-9a-fA-F]{40}$ ]]; then
    echo 'error: revision must be an exact 40-character commit SHA' >&2
    exit 2
fi

case "$operation" in
    ci)
        if [[ -n "$target" ]]; then
            echo 'error: ci does not accept a target' >&2
            exit 2
        fi
        ;;
    test|verify)
        if [[ ! "$target" =~ ^[A-Za-z0-9._-]{1,128}$ ]]; then
            echo 'error: target must be 1-128 characters from [A-Za-z0-9._-]' >&2
            exit 2
        fi
        ;;
    *)
        echo 'error: operation must be ci, test, or verify' >&2
        exit 2
        ;;
esac

if [[ ! "$request_id" =~ ^[A-Za-z0-9._-]{1,80}$ ]]; then
    echo 'error: request_id must be 1-80 characters from [A-Za-z0-9._-]' >&2
    exit 2
fi

: "${GITHUB_OUTPUT:?GITHUB_OUTPUT is required}"
printf 'revision=%s\n' "$revision" >> "$GITHUB_OUTPUT"
printf 'operation=%s\n' "$operation" >> "$GITHUB_OUTPUT"
printf 'target=%s\n' "$target" >> "$GITHUB_OUTPUT"
printf 'request_id=%s\n' "$request_id" >> "$GITHUB_OUTPUT"
