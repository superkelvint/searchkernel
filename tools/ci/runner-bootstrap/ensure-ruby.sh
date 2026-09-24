#!/usr/bin/env bash
set -euo pipefail

RUBY_VERSION=${SEARCHKERNEL_RUBY_VERSION:-3.3.6}
RUBY_SHA256=${SEARCHKERNEL_RUBY_SHA256:-8dc48fffaf270f86f1019053f28e51e4da4cce32a36760a0603a9aee67d7fd8d}
CACHE_ROOT=${SEARCHKERNEL_RUBY_CACHE_ROOT:-${XDG_CACHE_HOME:-$HOME/.cache}/searchkernel/ruby}
PREFIX="$CACHE_ROOT/$RUBY_VERSION"
RUBY_SERIES="${RUBY_VERSION%.*}"
SOURCE_URL=${SEARCHKERNEL_RUBY_SOURCE_URL:-https://cache.ruby-lang.org/pub/ruby/$RUBY_SERIES/ruby-$RUBY_VERSION.tar.gz}

ruby_usable() {
    local ruby_bin=$1
    [[ -x "$ruby_bin" ]] || return 1
    "$ruby_bin" -rrubygems -e 'exit Gem::Version.new(RUBY_VERSION) >= Gem::Version.new("3.1") ? 0 : 1' >/dev/null 2>&1
}

publish_path() {
    local bin_dir=$1
    if [[ -n "${GITHUB_PATH:-}" ]]; then
        printf '%s\n' "$bin_dir" >> "$GITHUB_PATH"
    fi
    printf 'Using Ruby: '
    "$bin_dir/ruby" --version
    "$bin_dir/gem" --version >/dev/null
}

if system_ruby=$(command -v ruby 2>/dev/null) && ruby_usable "$system_ruby"; then
    system_bin=$(dirname "$system_ruby")
    [[ -x "$system_bin/gem" ]] || {
        echo "Ruby is usable but gem is unavailable beside it: $system_bin" >&2
        exit 1
    }
    publish_path "$system_bin"
    exit 0
fi

mkdir -p "$CACHE_ROOT"
exec 9>"$CACHE_ROOT/.ruby-$RUBY_VERSION.lock"
command -v flock >/dev/null 2>&1 && flock 9

if ruby_usable "$PREFIX/bin/ruby" && [[ -x "$PREFIX/bin/gem" ]]; then
    publish_path "$PREFIX/bin"
    exit 0
fi

if [[ "${SEARCHKERNEL_RUBY_SKIP_DEP_INSTALL:-0}" != "1" ]]; then
    sudo -n apt-get install -y --no-install-recommends \
        libssl-dev libreadline-dev zlib1g-dev libyaml-dev libffi-dev \
        libgdbm-dev libncurses-dev
fi

for command in curl sha256sum tar make cc; do
    command -v "$command" >/dev/null 2>&1 || {
        echo "Ruby bootstrap requires '$command'" >&2
        exit 1
    }
done

work=$(mktemp -d "$CACHE_ROOT/.ruby-$RUBY_VERSION.build.XXXXXX")
install_root="$CACHE_ROOT/.ruby-$RUBY_VERSION.install.$BASHPID"
cleanup() {
    rm -rf "$work" "$install_root"
}
trap cleanup EXIT INT TERM

archive="$work/ruby-$RUBY_VERSION.tar.gz"
curl --fail --location --silent --show-error "$SOURCE_URL" --output "$archive"
printf '%s  %s\n' "$RUBY_SHA256" "$archive" | sha256sum --check --status

mkdir -p "$work/source"
tar -xzf "$archive" --strip-components=1 -C "$work/source"
(
    cd "$work/source"
    ./configure \
        --prefix="$install_root" \
        --disable-install-doc \
        --disable-yjit
    make -j"$(nproc 2>/dev/null || printf '2')"
    make install
)

ruby_usable "$install_root/bin/ruby" || {
    echo "pinned Ruby bootstrap produced an unusable interpreter" >&2
    exit 1
}
[[ -x "$install_root/bin/gem" ]] || {
    echo "pinned Ruby bootstrap produced no gem executable" >&2
    exit 1
}

rm -rf "$PREFIX"
mv "$install_root" "$PREFIX"
trap - EXIT INT TERM
rm -rf "$work"
publish_path "$PREFIX/bin"
