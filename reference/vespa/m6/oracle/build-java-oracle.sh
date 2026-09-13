#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M6="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$M6/../../.." && pwd)"
VESPA_ROOT="$ROOT/upstream/vespa"
BUILD="$M6/oracle/target"
TOOLS="$BUILD/tools"
source "$ROOT/reference/vespa/common/verify-common.sh"

fail() { echo "m6 java oracle: $*" >&2; exit 1; }

[[ -f "$VESPA_ROOT/pom.xml" ]] || fail "missing pinned Vespa source: $VESPA_ROOT"
require_pinned_vespa
actual="$(git -C "$VESPA_ROOT" rev-parse HEAD)"

mkdir -p "$BUILD/classes" "$TOOLS"

bad_target_file="$(find "$VESPA_ROOT" -path '*/target/*' -type f ! -writable -print -quit 2>/dev/null || true)"
if [[ -n "$bad_target_file" ]]; then
  fail "unwritable Maven build artifact: $bad_target_file
This usually means an earlier Docker/root build owns target/ files. Repair generated-target ownership once with:
  find '$VESPA_ROOT' -type d -name target -prune -exec sudo chown -R \"$USER\":\"$(id -gn)\" {} +"
fi

MAVEN_VERSION="$(sed -n 's/^wanted_mvn_version=\([^[:space:]]*\).*/\1/p' "$VESPA_ROOT/bootstrap.sh" | head -n1)"
[[ -n "$MAVEN_VERSION" ]] || fail "could not determine pinned Maven version"
MAVEN_HOME="$TOOLS/apache-maven-$MAVEN_VERSION"
MAVEN="$MAVEN_HOME/bin/mvn"

if [[ ! -x "$MAVEN" ]]; then
  archive="$TOOLS/apache-maven-$MAVEN_VERSION-bin.tar.gz"
  url="https://repo.maven.apache.org/maven2/org/apache/maven/apache-maven/$MAVEN_VERSION/apache-maven-$MAVEN_VERSION-bin.tar.gz"
  echo "Installing Maven $MAVEN_VERSION for M6 oracle" >&2
  if command -v curl >/dev/null; then
    curl --fail --location --retry 3 -o "$archive" "$url"
  else
    wget -O "$archive" "$url"
  fi
  tar -xzf "$archive" -C "$TOOLS"
fi

mvn_install() {
  "$MAVEN" --batch-mode --no-snapshot-updates \
    -Dmaven.test.skip=true -Dmaven.javadoc.skip=true -Dmaven.source.skip=true \
    install "$@"
}

STAMP="$BUILD/config-model-revision.txt"
if [[ ! -f "$STAMP" || "$(cat "$STAMP")" != "$actual" ]]; then
  echo "Bootstrapping pinned Vespa Maven metadata/plugins" >&2
  (cd "$VESPA_ROOT/dependency-versions" && mvn_install)
  (cd "$VESPA_ROOT/container-dependency-versions" && mvn_install)
  (cd "$VESPA_ROOT/parent" && mvn_install)
  (cd "$VESPA_ROOT" && mvn_install -N)
  (cd "$VESPA_ROOT" && mvn_install -f maven-plugins/pom.xml)

  echo "Building pinned config-model dependency closure" >&2
  "$MAVEN" --batch-mode --no-snapshot-updates \
    -f "$VESPA_ROOT/pom.xml" -pl config-model -am \
    -Dmaven.test.skip=true -Dmaven.javadoc.skip=true -Dmaven.source.skip=true \
    install

  printf '%s\n' "$actual" > "$STAMP"
fi

# config-model is an OSGi/container module. Some APIs it references are Maven
# provided-scope dependencies. A standalone oracle needs both compile/provided
# and runtime dependencies. Regenerating these classpath files is cheap once
# the pinned dependency closure is installed.
COMPILE_CP_FILE="$BUILD/classpath-compile.txt"
RUNTIME_CP_FILE="$BUILD/classpath-runtime.txt"

"$MAVEN" --batch-mode --no-snapshot-updates \
  -f "$VESPA_ROOT/config-model/pom.xml" \
  -DincludeScope=compile \
  -Dmdep.outputFile="$COMPILE_CP_FILE" \
  dependency:build-classpath

"$MAVEN" --batch-mode --no-snapshot-updates \
  -f "$VESPA_ROOT/config-model/pom.xml" \
  -DincludeScope=runtime \
  -Dmdep.outputFile="$RUNTIME_CP_FILE" \
  dependency:build-classpath

[[ -s "$COMPILE_CP_FILE" ]] || fail "missing generated compile/provided classpath"
[[ -s "$RUNTIME_CP_FILE" ]] || fail "missing generated runtime classpath"

CP="$VESPA_ROOT/config-model/target/classes:$(cat "$COMPILE_CP_FILE"):$(cat "$RUNTIME_CP_FILE")"

check_class() {
  local class_path="$1"
  local found=1
  IFS=':' read -r -a entries <<< "$CP"
  for entry in "${entries[@]}"; do
    if [[ -d "$entry" && -f "$entry/$class_path" ]]; then
      found=0
      break
    elif [[ -f "$entry" && "$entry" == *.jar ]]; then
      if jar tf "$entry" 2>/dev/null | grep -Fxq "$class_path"; then
        found=0
        break
      fi
    fi
  done
  [[ "$found" -eq 0 ]] || fail "required class missing from standalone oracle classpath: $class_path"
}

check_class "com/yahoo/config/model/api/ModelContext.class"
check_class "com/yahoo/io/reader/NamedReader.class"

rm -rf "$BUILD/classes"
mkdir -p "$BUILD/classes"
javac -Xlint:all,-path -Werror -cp "$CP" -d "$BUILD/classes" \
  "$M6/oracle/src/searchkernel/m6/SchemaOracle.java"
printf '%s\n' "$CP" > "$BUILD/runtime-classpath.txt"
