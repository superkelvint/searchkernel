#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
M3="$ROOT/reference/vespa/m3"
VESPA_ROOT="${VESPA_ROOT:-$ROOT/upstream/vespa}"
BUILD="$M3/oracle/target"
TOOLS="$BUILD/tools"
source "$ROOT/reference/vespa/common/verify-common.sh"

fail() {
  echo "m3 java oracle: $*" >&2
  exit 1
}

[[ -f "$VESPA_ROOT/pom.xml" ]] || fail "missing pinned Vespa source: $VESPA_ROOT/pom.xml"
[[ -f "$VESPA_ROOT/bootstrap.sh" ]] || fail "missing pinned Vespa bootstrap.sh"
[[ -f "$VESPA_ROOT/opennlp-linguistics/pom.xml" ]] || fail "missing pinned Vespa opennlp-linguistics module"
[[ -x "$VESPA_ROOT/dist/getversionmap.sh" ]] || fail "missing executable Vespa dist/getversionmap.sh"
[[ "$(cd "$VESPA_ROOT" && pwd -P)" == "$(cd "$ROOT/upstream/vespa" && pwd -P)" ]] || \
  fail "M3 oracle must use the repository's pinned Vespa checkout"
require_pinned_vespa

mkdir -p "$BUILD/classes" "$TOOLS"

REVISION="$(git -C "$VESPA_ROOT" rev-parse HEAD 2>/dev/null || printf 'not-a-git-checkout')"
EXPECTED_REVISION="$(expected_vespa_revision)"
[[ "$REVISION" == "$EXPECTED_REVISION" ]] || fail "expected Vespa $EXPECTED_REVISION, found $REVISION"
STAMP="$BUILD/vespa-revision.txt"

# Use the Maven version declared by the pinned Vespa checkout. Do not call
# bootstrap.sh here: bootstrap.sh first tries to create ./mvnw when /usr/bin/mvn
# is a different version, and that wrapper-generation step is not needed for
# this oracle build.
MAVEN_VERSION="$(sed -n 's/^wanted_mvn_version=\([^[:space:]]*\).*/\1/p' "$VESPA_ROOT/bootstrap.sh" | head -n 1)"
[[ -n "$MAVEN_VERSION" ]] || fail "could not determine wanted_mvn_version from pinned Vespa bootstrap.sh"

MAVEN_HOME="$TOOLS/apache-maven-$MAVEN_VERSION"
MAVEN="$MAVEN_HOME/bin/mvn"

install_maven() {
  local archive="$TOOLS/apache-maven-$MAVEN_VERSION-bin.tar.gz"
  local url="https://repo.maven.apache.org/maven2/org/apache/maven/apache-maven/$MAVEN_VERSION/apache-maven-$MAVEN_VERSION-bin.tar.gz"

  if [[ -x "$MAVEN" ]]; then
    return
  fi

  echo "Installing Maven $MAVEN_VERSION for the M3 oracle build"

  if command -v curl >/dev/null 2>&1; then
    curl --fail --location --retry 3 --output "$archive" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -O "$archive" "$url"
  else
    fail "need curl or wget to download Maven $MAVEN_VERSION"
  fi

  rm -rf "$MAVEN_HOME"
  tar -xzf "$archive" -C "$TOOLS"
  [[ -x "$MAVEN" ]] || fail "downloaded Maven archive did not create $MAVEN"
}

install_maven

actual_maven_version="$($MAVEN -version | awk '$1 == "Apache" && $2 == "Maven" { print $3; exit }')"
[[ "$actual_maven_version" == "$MAVEN_VERSION" ]] || \
  fail "expected Maven $MAVEN_VERSION, got ${actual_maven_version:-unknown}"

mvn_install() {
  "$MAVEN" \
    --batch-mode \
    --no-snapshot-updates \
    -Dmaven.wagon.http.retryHandler.count=5 \
    -Dmaven.test.skip=true \
    -Dmaven.javadoc.skip=true \
    -Dmaven.source.skip=true \
    clean install \
    "$@"
}

if [[ ! -f "$STAMP" || "$(cat "$STAMP")" != "$REVISION" ]]; then
  rm -rf "$BUILD/classes" "$BUILD/classpath.txt" "$BUILD/runtime-classpath.txt"
  mkdir -p "$BUILD/classes"

  echo "Bootstrapping pinned Vespa Maven metadata/plugins with Maven $MAVEN_VERSION"

  # Bootstrap only the Maven metadata/plugins required by the Java build.
  # Vespa's bootstrap.sh also generates dist/vtag.map, but that file is a
  # core/native/CMake build artifact.  The M3 Java oracle does not need it, and
  # the verifier must not mutate the pinned upstream checkout unnecessarily.

  (
    cd "$VESPA_ROOT/dependency-versions"
    mvn_install
  )

  (
    cd "$VESPA_ROOT/container-dependency-versions"
    mvn_install
  )

  (
    cd "$VESPA_ROOT/parent"
    mvn_install
  )

  (
    cd "$VESPA_ROOT"
    mvn_install -N
    mvn_install -f maven-plugins/pom.xml
  )

  echo "Building the pinned Vespa dependency closure for opennlp-linguistics"
  "$MAVEN" \
    --batch-mode \
    --no-snapshot-updates \
    -f "$VESPA_ROOT/pom.xml" \
    -pl opennlp-linguistics \
    -am \
    -Dmaven.test.skip=true \
    -Dmaven.javadoc.skip=true \
    -Dmaven.source.skip=true \
    install

  "$MAVEN" \
    --batch-mode \
    --no-snapshot-updates \
    -f "$VESPA_ROOT/opennlp-linguistics/pom.xml" \
    -DincludeScope=test \
    -Dmdep.outputFile="$BUILD/classpath.txt" \
    dependency:build-classpath

  printf '%s\n' "$REVISION" > "$STAMP"
fi

[[ -s "$BUILD/classpath.txt" ]] || fail "oracle dependency classpath was not generated"

CP="$VESPA_ROOT/opennlp-linguistics/target/classes:$(cat "$BUILD/classpath.txt")"

javac -Xlint:all,-path -Werror \
  -cp "$CP" \
  -d "$BUILD/classes" \
  "$M3/oracle/src/searchkernel/m3/LinguisticsOracle.java"

printf '%s\n' "$CP" > "$BUILD/runtime-classpath.txt"
