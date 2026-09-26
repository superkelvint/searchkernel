# syntax=docker/dockerfile:1.7

ARG TOOLCHAIN_IMAGE
ARG VESPA_IMAGE

FROM ${VESPA_IMAGE} AS vespa
FROM ${TOOLCHAIN_IMAGE}

SHELL ["/bin/bash", "-o", "pipefail", "-c"]
USER root

ARG TOOLCHAIN_ID
ARG VESPA_COMMIT
ARG VESPA_BUNDLE_ID

LABEL org.opencontainers.image.source="https://github.com/superkelvint/searchkernel"
LABEL io.searchkernel.toolchain-id="${TOOLCHAIN_ID}"
LABEL io.searchkernel.vespa-commit="${VESPA_COMMIT}"
LABEL io.searchkernel.vespa-bundle-id="${VESPA_BUNDLE_ID}"

# Keep the large Vespa payload as an independent linked layer. Changing the
# language/toolchain image does not require repacking the Vespa bundle, and a
# Vespa bump does not rebuild the language/toolchain image.
COPY --link --from=vespa /opt/searchkernel/native-sdk-cache/ /opt/searchkernel/native-sdk-cache/

RUN cache="/opt/searchkernel/native-sdk-cache/ci/x86_64-unknown-linux-gnu" \
    && base="$cache/objects/${VESPA_COMMIT}" \
    && toolchain="$cache/toolchains/${VESPA_COMMIT}" \
    && test -f "$base/manifest.json" \
    && test -f "$toolchain/upstream/vespa/.searchkernel-vespa-artifact.json" \
    && mkdir -p "$cache/checkouts" \
    && ln -sfn "$toolchain/upstream/vespa" "$cache/checkouts/${VESPA_COMMIT}" \
    && "${PYTHON}" - "$base/manifest.json" "$toolchain/upstream/vespa/.searchkernel-vespa-artifact.json" "${VESPA_COMMIT}" <<'PY'
import json
import sys

base_path, marker_path, commit = sys.argv[1:]
with open(base_path, encoding="utf-8") as stream:
    base = json.load(stream)
with open(marker_path, encoding="utf-8") as stream:
    marker = json.load(stream)

if base.get("kind") != "vespa-base":
    raise SystemExit("bundled native object is not a Vespa base")
if base.get("vespa_commit") != commit:
    raise SystemExit("bundled native object does not match VESPA_COMMIT")

expected = {
    "build_environment": "ci",
    "native_target": "x86_64-unknown-linux-gnu",
    "vespa_commit": commit,
}
for key, value in expected.items():
    if marker.get(key) != value:
        raise SystemExit(
            f"bundled native toolchain {key} mismatch: expected {value!r}, found {marker.get(key)!r}"
        )
if marker.get("format_version") != 2:
    raise SystemExit(
        f"bundled native toolchain format_version mismatch: expected 2, found {marker.get('format_version')!r}"
    )
generation = marker.get("artifact_generation")
if not isinstance(generation, int) or generation < 1:
    raise SystemExit("bundled native toolchain has no valid artifact_generation")
if marker.get("adapter_static_link_closure") is not True:
    raise SystemExit("bundled native toolchain does not certify adapter static-link closure")
if marker.get("source") != "full-sdk-fingerprint":
    raise SystemExit(
        f"bundled native toolchain source mismatch: {marker.get('source')!r}"
    )
PY

# Composition-time compatibility gate. Toolchain and Vespa may advance
# independently, but an incompatible pair is never published as a CI base.
RUN base="/opt/searchkernel/native-sdk-cache/ci/x86_64-unknown-linux-gnu/objects/${VESPA_COMMIT}" \
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
    && test "$("$CC" -dumpfullversion -dumpversion | cut -d. -f1)" -ge 15 \
    && test "$("$CXX" -dumpfullversion -dumpversion | cut -d. -f1)" -ge 15 \
    && "$CXX" -std=gnu++23 -Wmismatched-tags -x c++ -fsyntax-only - <<< 'struct Smoke {}; int main() { Smoke value; (void)value; }' \
    && stdcpp="$("$CXX" -print-file-name=libstdc++.so)" \
    && case "$stdcpp" in "$GCC_TOOLSET_ROOT"/*) ;; *) echo "unexpected libstdc++: $stdcpp" >&2; exit 1;; esac \
    && command -v rg \
    && rg --version | head -n 1 \
    && command -v gh \
    && gh --version | head -n 1 \
    && command -v node \
    && command -v npm \
    && command -v php \
    && command -v ruby \
    && command -v "${PYTHON}" \
    && command -v rustc \
    && command -v cargo \
    && cargo llvm-cov --version \
    && command -v flatc \
    && command -v cmake \
    && command -v clang \
    && c++ --version | head -n 1 \
    && [[ "$(c++ -dumpfullversion)" == 15.* ]] \
    && test -f "$(c++ -print-file-name=libstdc++.so)" \
    && /usr/bin/python3 -c "import clang.cindex" \
    && "${PYTHON}" -c "import build, flatbuffers, grpc, google.protobuf"

WORKDIR /work
CMD ["/bin/bash"]
