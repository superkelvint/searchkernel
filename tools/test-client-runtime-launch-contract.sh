#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

CLIENT_RUNTIME="$ROOT/tools/client-e2e-runtime.sh"
CLIENT_HOST="$ROOT/tools/client-e2e-host.sh"
PYTHON_E2E="$ROOT/tools/run-python-native-e2e.sh"
TS_E2E="$ROOT/clients/typescript/scripts/run-real-e2e.sh"
PHP_E2E="$ROOT/clients/php/examples/run_searchkernel_client_api_e2e.sh"
RUBY_E2E="$ROOT/clients/ruby/examples/run_searchkernel_client_api_e2e.sh"
TS_INSTALLED_E2E="$ROOT/clients/typescript/scripts/verify-installed-managed-local-e2e.cjs"
PYTHON_GRPC_E2E="$ROOT/clients/python/tests/test_grpc_e2e.py"
DEV="$ROOT/dev"

[[ -x "$CLIENT_HOST" ]] || {
    echo "client E2E canonical host launcher is missing or not executable: $CLIENT_HOST" >&2
    exit 1
}

grep -F 'tools/run-runtime.sh" native "$SEARCHKERNEL_LOCAL_HOST_BINARY"' "$CLIENT_HOST" >/dev/null || {
    echo "client E2E host launcher does not delegate daemon startup to canonical native execution" >&2
    exit 1
}

grep -F 'export SEARCHKERNEL_LOCAL_HOST_BINARY="$host"' "$CLIENT_RUNTIME" >/dev/null || {
    echo "client E2E runtime does not preserve the exact-head daemon binary separately from its launcher" >&2
    exit 1
}
grep -F 'export SEARCHKERNEL_LOCAL_HOST="$root/tools/client-e2e-host.sh"' "$CLIENT_RUNTIME" >/dev/null || {
    echo "client E2E runtime does not expose the canonical daemon launcher" >&2
    exit 1
}
grep -F 'export SEARCHKERNEL_RUNTIME_LIB_DIRS=' "$CLIENT_RUNTIME" >/dev/null || {
    echo "client E2E runtime does not export the canonical native runtime closure" >&2
    exit 1
}
if grep -F 'export LD_LIBRARY_PATH=' "$CLIENT_RUNTIME" >/dev/null; then
    echo "client E2E runtime still mutates LD_LIBRARY_PATH directly" >&2
    exit 1
fi

grep -F 'RUNTIME="$ROOT/tools/run-runtime.sh"' "$PYTHON_E2E" >/dev/null || {
    echo "Python native E2E does not select the canonical runtime launcher" >&2
    exit 1
}
grep -F '"$RUNTIME" python' "$PYTHON_E2E" >/dev/null || {
    echo "Python native E2E bypasses the canonical Python launcher" >&2
    exit 1
}
grep -F '"$RUNTIME" npm' "$TS_E2E" >/dev/null || {
    echo "TypeScript E2E bypasses the canonical npm launcher" >&2
    exit 1
}
grep -F '"$RUNTIME" node' "$TS_E2E" >/dev/null || {
    echo "TypeScript E2E bypasses the canonical Node launcher" >&2
    exit 1
}
grep -F '"$RUNTIME" php' "$PHP_E2E" >/dev/null || {
    echo "PHP E2E bypasses the canonical PHP launcher" >&2
    exit 1
}
grep -F '"$RUNTIME" python' "$PHP_E2E" >/dev/null || {
    echo "PHP E2E helper Python bypasses the canonical Python launcher" >&2
    exit 1
}
grep -F '"$RUNTIME" ruby' "$RUBY_E2E" >/dev/null || {
    echo "Ruby E2E bypasses the canonical Ruby launcher" >&2
    exit 1
}
grep -F 'const npm = process.env.NPM;' "$TS_INSTALLED_E2E" >/dev/null || {
    echo "installed TypeScript E2E does not inherit the npm selected by the canonical Node launcher" >&2
    exit 1
}
grep -F 'SEARCHKERNEL_LOCAL_HOST' "$PYTHON_GRPC_E2E" >/dev/null || {
    echo "Python gRPC E2E does not launch the prepared canonical host wrapper" >&2
    exit 1
}
if grep -F 'LD_LIBRARY_PATH' "$PYTHON_GRPC_E2E" >/dev/null; then
    echo "Python gRPC E2E still constructs its own loader path" >&2
    exit 1
fi

for fn in run_python_tests run_typescript_tests run_php_tests run_ruby_tests; do
    body=$(sed -n "/^$fn() {/,/^}/p" "$DEV")
    grep -F 'tools/run-runtime.sh' <<<"$body" >/dev/null || {
        echo "./dev $fn does not route language execution through the canonical runtime launcher" >&2
        exit 1
    }
done

echo "client runtime launch contract PASS"
