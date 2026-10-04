# syntax=docker/dockerfile:1.7

ARG BASE_IMAGE=docker.io/vespaengine/vespa-dev-almalinux-8:latest
FROM ${BASE_IMAGE}

SHELL ["/bin/bash", "-o", "pipefail", "-c"]
USER root

ARG NODE_VERSION=22.20.0
ARG RUBY_VERSION=3.3.9
ARG RUST_TOOLCHAIN=1.98.0
ARG FLATBUFFERS_VERSION=25.12.19
ARG COMPOSER_VERSION=2.8.12
ARG SCCACHE_VERSION=0.17.0
ARG SCCACHE_SHA256=67c4a96dd237c1f518f6b36083f270f9976d516f1e57fce891755ea782e50006

ENV GCC_TOOLSET_ROOT="/opt/rh/gcc-toolset-15/root/usr" \
    PATH="/root/.cargo/bin:/opt/rh/gcc-toolset-15/root/usr/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin" \
    CC="/opt/rh/gcc-toolset-15/root/usr/bin/gcc" \
    CXX="/opt/rh/gcc-toolset-15/root/usr/bin/g++" \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="/opt/rh/gcc-toolset-15/root/usr/bin/gcc" \
    AR="/opt/rh/gcc-toolset-15/root/usr/bin/ar" \
    RANLIB="/opt/rh/gcc-toolset-15/root/usr/bin/ranlib" \
    LIBRARY_PATH="/opt/rh/gcc-toolset-15/root/usr/lib64:/opt/rh/gcc-toolset-15/root/usr/lib" \
    LD_LIBRARY_PATH="/opt/rh/gcc-toolset-15/root/usr/lib64:/opt/rh/gcc-toolset-15/root/usr/lib" \
    PYTHON="/usr/bin/python3.11" \
    SEARCHKERNEL_BUILD_ENV="ci" \
    SK_NATIVE_TARGET="x86_64-unknown-linux-gnu" \
    SK_NATIVE_CACHE_DIR="/opt/searchkernel/native-sdk-cache" \
    SK_NATIVE_DISABLE_PUBLISHED_BASE="1" \
    SEARCHKERNEL_TYPESCRIPT_NODE_MODULES="/opt/searchkernel/typescript/node_modules"

ENV PLAYWRIGHT_BROWSERS_PATH="/opt/searchkernel/playwright"

# Stable OS/tooling surface. This image is published only when the CI toolchain
# changes (for example, adding a client language or bumping a compiler/runtime).
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

RUN curl --fail --location --retry 5 --silent --show-error \
        "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz" \
        -o /tmp/node.tar.xz \
    && tar -xJf /tmp/node.tar.xz -C /usr/local --strip-components=1 \
    && rm -f /tmp/node.tar.xz \
    && node -e 'const [M,m]=process.versions.node.split(".").map(Number); if (M < 20 || (M === 20 && m < 19)) process.exit(1)' \
    && node --version \
    && npm --version

# TypeScript package/tooling dependencies belong to the reusable toolchain
# axis. Verification source snapshots deliberately exclude node_modules, so
# install the exact lockfile once into an image-owned immutable location.
COPY clients/typescript/package.json clients/typescript/package-lock.json /opt/searchkernel/typescript/
RUN npm ci --prefix /opt/searchkernel/typescript --no-audit --no-fund \
    && test -x /opt/searchkernel/typescript/node_modules/grpc-tools/bin/protoc \
    && test -x /opt/searchkernel/typescript/node_modules/grpc-tools/bin/grpc_node_plugin \
    && test -x /opt/searchkernel/typescript/node_modules/protoc-gen-grpc-web/bin/protoc-gen-grpc-web \
    && test -x /opt/searchkernel/typescript/node_modules/.bin/tsc \
    && test -x /opt/searchkernel/typescript/node_modules/.bin/vitest

# InstantSearch acceptance consumes this declared, pinned browser prerequisite.
# Normal checks never install operating-system packages or browser binaries.
RUN dnf install -y alsa-lib atk at-spi2-atk at-spi2-core cups-libs libdrm libX11 libXcomposite libXdamage libXext libXfixes libXrandr libxcb libxkbcommon mesa-libgbm nss pango cairo && dnf clean all
COPY integrations/instantsearch/package.json integrations/instantsearch/package-lock.json /opt/searchkernel/instantsearch/
RUN npm ci --prefix /opt/searchkernel/instantsearch && \
    npm exec --prefix /opt/searchkernel/instantsearch -- playwright install chromium

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

RUN curl --fail --location --retry 5 --silent --show-error \
        "https://getcomposer.org/download/${COMPOSER_VERSION}/composer.phar" \
        -o /usr/local/bin/composer \
    && chmod +x /usr/local/bin/composer \
    && php -r 'exit(version_compare(PHP_VERSION, "8.1.0", ">=") ? 0 : 1);' \
    && php --version | head -n 1 \
    && composer --version

RUN work="$(mktemp -d /tmp/sccache.XXXXXX)" \
    && asset="sccache-v${SCCACHE_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
    && curl --fail --location --retry 5 --silent --show-error \
        "https://github.com/mozilla/sccache/releases/download/v${SCCACHE_VERSION}/$asset" \
        -o "$work/$asset" \
    && printf '%s  %s\n' "$SCCACHE_SHA256" "$work/$asset" | sha256sum -c - \
    && tar -xzf "$work/$asset" -C "$work" \
    && install -m 0755 \
        "$work/sccache-v${SCCACHE_VERSION}-x86_64-unknown-linux-musl/sccache" \
        /usr/local/bin/sccache \
    && rm -rf "$work" \
    && test "$(sccache --version)" = "sccache ${SCCACHE_VERSION}"

RUN curl --proto "=https" --tlsv1.2 --retry 5 --silent --show-error --fail \
        https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain "${RUST_TOOLCHAIN}" --no-modify-path \
    && rustup component add rustfmt clippy llvm-tools-preview --toolchain "${RUST_TOOLCHAIN}" \
    && cargo install cargo-llvm-cov --locked --version 0.9.1 \
    && rustup default "${RUST_TOOLCHAIN}" \
    && rustc --version \
    && cargo --version \
    && rustfmt --version \
    && cargo clippy --version \
    && cargo llvm-cov --version

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

RUN "${PYTHON}" -m pip install --no-cache-dir --upgrade pip setuptools wheel \
    && "${PYTHON}" -m pip install --no-cache-dir \
        flatbuffers==25.12.19 \
        grpcio==1.84.0 \
        grpcio-tools==1.84.0 \
        protobuf==7.35.1 \
    && "${PYTHON}" --version

# Toolchain-only smoke test. Native/Vespa compatibility is verified when this
# image is composed with a searchkernel-ci-vespa image.
RUN test "$("$CC" -dumpfullversion -dumpversion | cut -d. -f1)" -ge 15 \
    && test "$("$CXX" -dumpfullversion -dumpversion | cut -d. -f1)" -ge 15 \
    && test "$CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER" = "$CC" \
    && "$CXX" -std=gnu++23 -Wmismatched-tags -x c++ -fsyntax-only - <<< 'struct Smoke {}; int main() { Smoke value; (void)value; }' \
    && stdcpp="$("$CXX" -print-file-name=libstdc++.so)" \
    && case "$stdcpp" in "$GCC_TOOLSET_ROOT"/*) ;; *) echo "unexpected libstdc++: $stdcpp" >&2; exit 1;; esac \
    && command -v rg \
    && command -v gh \
    && command -v node \
    && command -v npm \
    && test -x "$SEARCHKERNEL_TYPESCRIPT_NODE_MODULES/grpc-tools/bin/protoc" \
    && test -x "$SEARCHKERNEL_TYPESCRIPT_NODE_MODULES/protoc-gen-grpc-web/bin/protoc-gen-grpc-web" \
    && test -x "$SEARCHKERNEL_TYPESCRIPT_NODE_MODULES/.bin/tsc" \
    && test -x "$SEARCHKERNEL_TYPESCRIPT_NODE_MODULES/.bin/vitest" \
    && command -v php \
    && command -v ruby \
    && command -v composer \
    && command -v "${PYTHON}" \
    && command -v rustc \
    && command -v cargo \
    && command -v sccache \
    && test "$(sccache --version)" = "sccache ${SCCACHE_VERSION}" \
    && cargo llvm-cov --version \
    && command -v flatc \
    && command -v cmake \
    && command -v clang \
    && [[ "$(c++ -dumpfullversion)" == 15.* ]] \
    && test -f "$(c++ -print-file-name=libstdc++.so)"

WORKDIR /work
CMD ["/bin/bash"]
