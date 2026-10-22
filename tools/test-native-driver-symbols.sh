#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
temporary=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-native-symbols.XXXXXX")
trap 'rm -rf -- "$temporary"' EXIT
mkdir -p "$temporary/base" "$temporary/vespa" "$temporary/sdk"
export SEARCHKERNEL_NATIVE_PROBE_BASE_SDK="$temporary/base"
export SEARCHKERNEL_NATIVE_PROBE_VESPA_ROOT="$temporary/vespa"
export SEARCHKERNEL_NATIVE_PROBE_SDK="$temporary/sdk"
export VESPA_ROOT="$temporary/vespa"
unset SK_NATIVE_SDK

cat > "$temporary/library.cc" <<'CPP'
extern "C" int sk_engine_search_generic() { return 0; }
extern "C" int sk_engine_search() { return 0; }
CPP
cat > "$temporary/exports.map" <<'MAP'
SK_SYMBOL_TEST_1 { global: sk_engine_search; sk_engine_search_generic; local: *; };
MAP
"${CXX:-c++}" -shared -fPIC "$temporary/library.cc" \
  -Wl,-soname,libsearchkernel_vespa.so \
  -Wl,--version-script="$temporary/exports.map" \
  -o "$temporary/libsearchkernel_vespa.so"

cat > "$temporary/generic.cc" <<'CPP'
extern "C" int sk_engine_search_generic();
int main() { return sk_engine_search_generic(); }
CPP
cat > "$temporary/exact.cc" <<'CPP'
extern "C" int sk_engine_search();
int main() { return sk_engine_search(); }
CPP
cat > "$temporary/defined.cc" <<'CPP'
extern "C" int sk_engine_search_generic();
extern "C" int sk_engine_search() { return 0; }
int main() { return sk_engine_search_generic() + sk_engine_search(); }
CPP
for name in generic exact defined; do
  "${CXX:-c++}" "$temporary/$name.cc" \
    -L"$temporary" -lsearchkernel_vespa -Wl,-rpath,"$temporary" \
    -Wl,--export-dynamic -o "$temporary/$name"
done

check_driver() {
  bash -euo pipefail -c 'source "$1"; shift; require_native_driver "$@"' \
    _ "$ROOT/reference/vespa/common/verify-common.sh" "$@"
}

# Real binaries and versioned dynamic references exercise nm's actual output.
check_driver "$temporary/exact" sk_engine_search
check_driver "$temporary/generic" sk_engine_search_generic
for driver in generic defined; do
  if check_driver "$temporary/$driver" sk_engine_search >"$temporary/out" 2>&1; then
    echo "accepted a non-imported exact symbol from $driver driver" >&2
    exit 1
  fi
  grep -F 'does not reference required symbol: sk_engine_search' "$temporary/out" >/dev/null
done
if check_driver "$temporary/generic" 'sk_engine_s.*' >"$temporary/out" 2>&1; then
  echo 'interpreted a required symbol as a regular expression' >&2
  exit 1
fi

# A tool failure must not become a pass even if it emits a matching line.
mkdir "$temporary/broken-bin"
cat > "$temporary/broken-bin/nm" <<'SH'
#!/usr/bin/env bash
printf 'sk_engine_search U\n'
exit 7
SH
chmod +x "$temporary/broken-bin/nm"
if PATH="$temporary/broken-bin:$PATH" check_driver "$temporary/exact" sk_engine_search \
    >"$temporary/out" 2>&1; then
  echo 'ignored a failed nm invocation' >&2
  exit 1
fi

echo 'PASS exact undefined versioned native symbols, prefix/export rejection, and nm failure handling'
