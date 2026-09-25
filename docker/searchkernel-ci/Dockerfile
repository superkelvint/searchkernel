# syntax=docker/dockerfile:1.7

# The normal Search Kernel image build starts from an already-published
# (toolchain, Vespa) composite base. Ordinary source changes therefore do not
# rebuild language runtimes, compilers, FlatBuffers, or repack the Vespa SDK.
ARG CI_BASE_IMAGE
FROM ${CI_BASE_IMAGE} AS runtime

ARG VESPA_COMMIT
ARG VESPA_BUNDLE_ID
ARG TOOLCHAIN_ID

LABEL org.opencontainers.image.source="https://github.com/superkelvint/searchkernel"
LABEL io.searchkernel.toolchain-id="${TOOLCHAIN_ID}"
LABEL io.searchkernel.vespa-commit="${VESPA_COMMIT}"
LABEL io.searchkernel.vespa-bundle-id="${VESPA_BUNDLE_ID}"

# Prove that the reusable base can rebuild the adapter from the exact Search
# Kernel source revision being published. BuildKit cache mounts accelerate this
# source-specific validation without introducing any mutable state into the
# resulting image or into CI runner containers.
FROM runtime AS native-validation
COPY --from=searchkernel-source / /tmp/searchkernel-source/
WORKDIR /tmp/searchkernel-source
RUN --mount=type=cache,id=searchkernel-ci-publish-cargo-registry,target=/root/.cargo/registry \
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
