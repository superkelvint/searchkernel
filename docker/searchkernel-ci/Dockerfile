# syntax=docker/dockerfile:1.7

# The final CI image contains only the reusable toolchain plus a tiny
# source-validation receipt. Vespa is an external immutable input: it is mounted
# from its own image during build-time validation and mounted separately by CI
# runners at runtime. Its ~1.8 GB payload is never copied into this image.
ARG TOOLCHAIN_IMAGE
ARG VESPA_IMAGE

FROM ${VESPA_IMAGE} AS vespa
FROM ${TOOLCHAIN_IMAGE} AS runtime

ARG TOOLCHAIN_ID
ARG VESPA_COMMIT
ARG VESPA_BUNDLE_ID
ARG CI_ENVIRONMENT_ID

LABEL org.opencontainers.image.source="https://github.com/superkelvint/searchkernel"
LABEL io.searchkernel.environment-id="${CI_ENVIRONMENT_ID}"
LABEL io.searchkernel.toolchain-id="${TOOLCHAIN_ID}"
LABEL io.searchkernel.vespa-commit="${VESPA_COMMIT}"
LABEL io.searchkernel.vespa-bundle-id="${VESPA_BUNDLE_ID}"

# Validate the exact toolchain + Vespa pair without copying any Vespa bytes into
# a committed image layer. The symlinks exist only in this intermediate stage.
FROM runtime AS environment-validation
RUN --mount=from=vespa,source=/opt/searchkernel/native-sdk-cache,target=/mnt/searchkernel-native-cache,ro \
    cache="/opt/searchkernel/native-sdk-cache/ci/x86_64-unknown-linux-gnu" \
    && mounted="/mnt/searchkernel-native-cache/ci/x86_64-unknown-linux-gnu" \
    && mkdir -p "$cache/checkouts" \
    && ln -s "$mounted/objects" "$cache/objects" \
    && ln -s "$mounted/toolchains" "$cache/toolchains" \
    && ln -s "$mounted/toolchains/${VESPA_COMMIT}/upstream/vespa" "$cache/checkouts/${VESPA_COMMIT}" \
    && base="$cache/objects/${VESPA_COMMIT}" \
    && toolchain="$cache/toolchains/${VESPA_COMMIT}" \
    && test -f "$base/manifest.json" \
    && test -f "$toolchain/upstream/vespa/.searchkernel-vespa-artifact.json" \
    && "${PYTHON}" - "$base/manifest.json" "$toolchain/upstream/vespa/.searchkernel-vespa-artifact.json" "${VESPA_COMMIT}" <<'PY'
import json
import sys

base_path, marker_path, commit = sys.argv[1:]
with open(base_path, encoding="utf-8") as stream:
    base = json.load(stream)
with open(marker_path, encoding="utf-8") as stream:
    marker = json.load(stream)

if base.get("kind") != "vespa-base":
    raise SystemExit("mounted native object is not a Vespa base")
if base.get("vespa_commit") != commit:
    raise SystemExit("mounted native object does not match VESPA_COMMIT")

expected = {
    "build_environment": "ci",
    "native_target": "x86_64-unknown-linux-gnu",
    "vespa_commit": commit,
}
for key, value in expected.items():
    if marker.get(key) != value:
        raise SystemExit(
            f"mounted native toolchain {key} mismatch: expected {value!r}, found {marker.get(key)!r}"
        )
if marker.get("format_version") != 2:
    raise SystemExit(
        f"mounted native toolchain format_version mismatch: expected 2, found {marker.get('format_version')!r}"
    )
if marker.get("adapter_static_link_closure") is not True:
    raise SystemExit("mounted native toolchain does not certify adapter static-link closure")
PY

RUN --mount=from=vespa,source=/opt/searchkernel/native-sdk-cache,target=/mnt/searchkernel-native-cache,ro \
    base="/opt/searchkernel/native-sdk-cache/ci/x86_64-unknown-linux-gnu/objects/${VESPA_COMMIT}" \
    && test -s "$base/lib/libvespalib.so" \
    && find "$base/lib" -maxdepth 1 -type f -name 'libvespa*.so*' -print -quit | grep -q . \
    && while IFS= read -r library; do \
        output="$(env -u LD_PRELOAD LD_LIBRARY_PATH="$base/lib" ldd "$library")"; \
        if grep -q '=> not found' <<<"$output"; then \
            echo "missing native dependency for $library" >&2; \
            echo "$output" >&2; \
            exit 1; \
        fi; \
    done < <(find "$base/lib" -maxdepth 1 -type f -name 'libvespa*.so*' -print) \
    && "$CXX" -std=gnu++23 -Wmismatched-tags -x c++ -fsyntax-only - <<< 'struct Smoke {}; int main() { Smoke value; (void)value; }'

# Rebuild the native adapter from the exact source revision while the Vespa
# image is mounted read-only. Cargo caches are scoped by CI_ENVIRONMENT_ID.
FROM environment-validation AS native-validation
COPY --from=searchkernel-source / /tmp/searchkernel-source/
WORKDIR /tmp/searchkernel-source
RUN --mount=from=vespa,source=/opt/searchkernel/native-sdk-cache,target=/mnt/searchkernel-native-cache,ro \
    --mount=type=cache,id=searchkernel-ci-publish-cargo-registry,target=/root/.cargo/registry \
    --mount=type=cache,id=searchkernel-ci-publish-cargo-git,target=/root/.cargo/git \
    --mount=type=cache,id=searchkernel-ci-publish-target,target=/tmp/searchkernel-target,sharing=locked \
    unset SK_NATIVE_SDK VESPA_ROOT VESPA_DEPS VESPA_LIBRARY_ROOT \
    && CARGO_TARGET_DIR=/tmp/searchkernel-target \
       cargo check -p searchkernel-vespa-bindings --features native \
    && touch /tmp/searchkernel-native-validation-pass

FROM runtime AS final
COPY --from=native-validation /tmp/searchkernel-native-validation-pass /opt/searchkernel/native-validation-pass
WORKDIR /work
CMD ["/bin/bash"]
