# syntax=docker/dockerfile:1.7

ARG BASE_IMAGE=docker.io/vespaengine/vespa-dev-almalinux-8:latest
FROM ${BASE_IMAGE} AS runtime

SHELL ["/bin/bash", "-o", "pipefail", "-c"]
USER root

ARG NODE_VERSION=22.20.0
ARG RUBY_VERSION=3.3.9
ARG RUST_TOOLCHAIN=1.98.0
ARG FLATBUFFERS_VERSION=25.12.19
ARG COMPOSER_VERSION=2.8.12
ARG VESPA_COMMIT

ENV PATH="/root/.cargo/bin:/opt/rh/gcc-toolset-15/root/usr/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin" \
    LD_LIBRARY_PATH="/opt/rh/gcc-toolset-15/root/usr/lib64:/opt/rh/gcc-toolset-15/root/usr/lib" \
    PYTHON="/usr/bin/python3.11" \
    SEARCHKERNEL_BUILD_ENV="ci" \
    SK_NATIVE_TARGET="x86_64-unknown-linux-gnu" \
    SK_NATIVE_CACHE_DIR="/opt/searchkernel/native-sdk-cache" \
    SK_NATIVE_DISABLE_PUBLISHED_BASE="1"

# Stable OS/tooling surface. Do this once when the image is built, never on a
# live runner.
RUN dnf install -y epel-release dnf-plugins-core \
    && dnf install -y \
        autoconf \
        automake \
        bzip2 \
        bzip2-devel \
        ca-certificates \
        clang \
        cmake \
        curl \
        diffutils \
        file \
        findutils \
        gcc \
        gcc-c++ \
        gdbm-devel \
        git \
        gzip \
        jq \
        libffi-devel \
        libyaml-devel \
        make \
        ncurses-devel \
        openssl-devel \
        patch \
        perl \
        procps-ng \
        python3-clang \
        python3.11 \
        python3.11-devel \
        python3.11-pip \
        readline-devel \
        ripgrep \
        rsync \
        tar \
        unzip \
        util-linux \
        which \
        xz \
        xz-devel \
        zip \
        zlib-devel \
    && dnf config-manager --add-repo https://cli.github.com/packages/rpm/gh-cli.repo \
    && dnf install -y gh \
    && dnf install -y https://rpms.remirepo.net/enterprise/remi-release-8.rpm \
    && dnf module reset -y php \
    && dnf module enable -y php:remi-8.3 \
    && dnf install -y php-cli php-mbstring \
    && dnf clean all \
    && rm -rf /var/cache/dnf

# Node is installed from the upstream binary rather than an AlmaLinux module:
# the module name alone does not guarantee the >=20.19 runtime required by the
# client capability gate.
RUN curl --fail --location --retry 5 --silent --show-error \
        "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz" \
        -o /tmp/node.tar.xz \
    && tar -xJf /tmp/node.tar.xz -C /usr/local --strip-components=1 \
    && rm -f /tmp/node.tar.xz \
    && node -e 'const [M,m]=process.versions.node.split(".").map(Number); if (M < 20 || (M === 20 && m < 19)) process.exit(1)' \
    && node --version \
    && npm --version

# Ruby 3.3 is built once into the immutable image. Do not rely on the old Ruby
# stream shipped by AlmaLinux 8.
RUN curl --fail --location --retry 5 --silent --show-error \
        "https://cache.ruby-lang.org/pub/ruby/3.3/ruby-${RUBY_VERSION}.tar.gz" \
        -o /tmp/ruby.tar.gz \
    && mkdir -p /tmp/ruby-src \
    && tar -xzf /tmp/ruby.tar.gz -C /tmp/ruby-src --strip-components=1 \
    && cd /tmp/ruby-src \
    && ./configure --disable-install-doc --with-openssl-dir=/usr \
    && make -j"$(nproc)" \
    && make install \
    && cd / \
    && rm -rf /tmp/ruby-src /tmp/ruby.tar.gz \
    && gem install bundler -v 2.6.9 --no-document \
    && ruby -e 'abort "Ruby >=3.3 required" if Gem::Version.new(RUBY_VERSION) < Gem::Version.new("3.3.0")' \
    && ruby --version \
    && bundle --version

# Composer is useful for PHP client/package checks even though the current
# client has no third-party runtime dependency.
RUN curl --fail --location --retry 5 --silent --show-error \
        "https://getcomposer.org/download/${COMPOSER_VERSION}/composer.phar" \
        -o /usr/local/bin/composer \
    && chmod +x /usr/local/bin/composer \
    && php -r 'exit(version_compare(PHP_VERSION, "8.1.0", ">=") ? 0 : 1);' \
    && php --version | head -n 1 \
    && composer --version

# Pin Rust rather than mutating a shared runner toolchain between jobs.
RUN curl --proto "=https" --tlsv1.2 --retry 5 --silent --show-error --fail \
        https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain "${RUST_TOOLCHAIN}" --no-modify-path \
    && rustup component add rustfmt clippy --toolchain "${RUST_TOOLCHAIN}" \
    && rustup default "${RUST_TOOLCHAIN}" \
    && rustc --version \
    && cargo --version \
    && rustfmt --version \
    && cargo clippy --version

# Search Kernel's canonical FlatBuffers C++ toolchain.
RUN work="$(mktemp -d /tmp/flatbuffers.XXXXXX)" \
    && curl --fail --location --retry 5 --silent --show-error \
        "https://github.com/google/flatbuffers/archive/refs/tags/v${FLATBUFFERS_VERSION}.tar.gz" \
        -o "$work/flatbuffers.tar.gz" \
    && mkdir -p "$work/src" "$work/build" \
    && tar -xzf "$work/flatbuffers.tar.gz" --strip-components=1 -C "$work/src" \
    && cmake \
        -S "$work/src" \
        -B "$work/build" \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX=/usr/local \
        -DCMAKE_INSTALL_LIBDIR=lib \
        -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
        -DFLATBUFFERS_BUILD_TESTS=OFF \
    && cmake --build "$work/build" --parallel "$(nproc)" \
    && cmake --install "$work/build" \
    && rm -rf "$work" \
    && test "$(flatc --version)" = "flatc version ${FLATBUFFERS_VERSION}"

# Runtime + code-generation dependencies for the Python client. The repository
# itself is mounted by the runner, so only reusable interpreter dependencies
# belong in this image.
RUN "${PYTHON}" -m pip install --no-cache-dir --upgrade pip setuptools wheel \
    && "${PYTHON}" -m pip install --no-cache-dir \
        flatbuffers==25.12.19 \
        grpcio==1.84.0 \
        grpcio-tools==1.84.0 \
        protobuf==7.35.1 \
    && "${PYTHON}" --version

# The publish script supplies these two named build contexts after producing a
# validated local native-base package. Nothing is downloaded from GitHub
# Releases while runners provision or execute jobs.
RUN test -n "${VESPA_COMMIT}"
COPY --from=searchkernel-native-base / /opt/searchkernel/native-sdk-cache/ci/x86_64-unknown-linux-gnu/objects/${VESPA_COMMIT}/
COPY --from=searchkernel-native-toolchain / /opt/searchkernel/native-sdk-cache/ci/x86_64-unknown-linux-gnu/toolchains/${VESPA_COMMIT}/

RUN cache="/opt/searchkernel/native-sdk-cache/ci/x86_64-unknown-linux-gnu" \
    && base="$cache/objects/${VESPA_COMMIT}" \
    && toolchain="$cache/toolchains/${VESPA_COMMIT}" \
    && mkdir -p "$cache/checkouts" \
    && ln -s "$toolchain/upstream/vespa" "$cache/checkouts/${VESPA_COMMIT}" \
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
PY

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
    done < <(find "$base/lib" -maxdepth 1 -type f -name 'libvespa*.so*' -print)

# Final capability smoke test. These checks intentionally fail the image build
# instead of allowing a broken runner to discover the problem later.
RUN command -v rg \
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
    && command -v flatc \
    && command -v cmake \
    && command -v clang \
    && c++ --version | head -n 1

# Prove that the bundled base/toolchain can rebuild the adapter from the exact
# Search Kernel source revision being published. The source exists only in this
# validation stage and is not retained in the final image.
FROM runtime AS native-validation
COPY --from=searchkernel-source / /tmp/searchkernel-source/
WORKDIR /tmp/searchkernel-source
RUN unset SK_NATIVE_SDK VESPA_ROOT VESPA_DEPS VESPA_LIBRARY_ROOT \
    && CARGO_TARGET_DIR=/tmp/searchkernel-target \
       cargo check -p searchkernel-vespa-bindings --features native \
    && touch /tmp/searchkernel-native-validation-pass

FROM runtime AS final
COPY --from=native-validation /tmp/searchkernel-native-validation-pass /opt/searchkernel/native-validation-pass
WORKDIR /work
CMD ["/bin/bash"]
