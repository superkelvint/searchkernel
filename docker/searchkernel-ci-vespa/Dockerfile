# syntax=docker/dockerfile:1.7

# The Vespa bundle is intentionally independent of the language/toolchain
# image. It contains only the immutable native runtime object and configured
# Vespa headers/static-link metadata for one pinned Vespa commit.
FROM scratch

ARG VESPA_COMMIT

LABEL org.opencontainers.image.source="https://github.com/superkelvint/searchkernel"
LABEL io.searchkernel.vespa-commit="${VESPA_COMMIT}"

COPY --link --from=searchkernel-native-base / /opt/searchkernel/native-sdk-cache/ci/x86_64-unknown-linux-gnu/objects/${VESPA_COMMIT}/
COPY --link --from=searchkernel-native-toolchain / /opt/searchkernel/native-sdk-cache/ci/x86_64-unknown-linux-gnu/toolchains/${VESPA_COMMIT}/
