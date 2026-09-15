#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M7="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$M7/../../.." && pwd)"
VESPA="$ROOT/upstream/vespa"
BUILD="$M7/oracle/target"
TOOLS="$BUILD/tools"
ORACLE_SOURCE="$M7/oracle/src/searchkernel/m7/QueryOracle.java"
source "$ROOT/reference/vespa/common/verify-common.sh"
fail(){ echo "m7 java oracle: $*" >&2; exit 1; }
require_pinned_vespa
actual="$(git -C "$VESPA" rev-parse HEAD)"
mkdir -p "$BUILD/classes" "$TOOLS"
bad="$(find "$VESPA" -path '*/target/*' -type f ! -writable -print -quit 2>/dev/null || true)"
[[ -z "$bad" ]] || fail "unwritable Maven artifact: $bad\nrepair generated target ownership before running the oracle"
MAVEN_VERSION="$(sed -n 's/^wanted_mvn_version=\([^[:space:]]*\).*/\1/p' "$VESPA/bootstrap.sh" | head -n1)"
[[ -n "$MAVEN_VERSION" ]] || fail "cannot determine pinned Maven version"
MAVEN_HOME="$TOOLS/apache-maven-$MAVEN_VERSION"; MAVEN="$MAVEN_HOME/bin/mvn"
if [[ ! -x "$MAVEN" ]]; then
  archive="$TOOLS/apache-maven-$MAVEN_VERSION-bin.tar.gz"
  url="https://repo.maven.apache.org/maven2/org/apache/maven/apache-maven/$MAVEN_VERSION/apache-maven-$MAVEN_VERSION-bin.tar.gz"
  curl --fail --location --retry 3 -o "$archive" "$url"
  tar -xzf "$archive" -C "$TOOLS"
fi
mvn_install(){ "$MAVEN" --batch-mode --no-snapshot-updates -Dmaven.test.skip=true -Dmaven.javadoc.skip=true -Dmaven.source.skip=true install "$@"; }
STAMP="$BUILD/query-revision.txt"
need_classpaths=0
if [[ ! -f "$STAMP" || "$(cat "$STAMP")" != "$actual" ]]; then
  (cd "$VESPA/dependency-versions" && mvn_install)
  (cd "$VESPA/container-dependency-versions" && mvn_install)
  (cd "$VESPA/parent" && mvn_install)
  (cd "$VESPA" && mvn_install -N)
  (cd "$VESPA" && mvn_install -f maven-plugins/pom.xml)
  "$MAVEN" --batch-mode --no-snapshot-updates -f "$VESPA/pom.xml" \
    -pl container-search,opennlp-linguistics -am \
    -Dmaven.test.skip=true -Dmaven.javadoc.skip=true -Dmaven.source.skip=true install
  printf '%s\n' "$actual" > "$STAMP"
  need_classpaths=1
fi
parts=("$VESPA/container-search/target/classes" "$VESPA/opennlp-linguistics/target/classes")
classpath_files=(
  "$BUILD/container-search-compile.classpath"
  "$BUILD/container-search-runtime.classpath"
  "$BUILD/opennlp-linguistics-compile.classpath"
  "$BUILD/opennlp-linguistics-runtime.classpath"
)
for f in "${classpath_files[@]}"; do
  [[ -s "$f" ]] || need_classpaths=1
done
if (( need_classpaths )); then
  for module in container-search opennlp-linguistics; do
    for scope in compile runtime; do
      f="$BUILD/$module-$scope.classpath"
      "$MAVEN" --batch-mode --no-snapshot-updates -f "$VESPA/$module/pom.xml" \
        -DincludeScope="$scope" -Dmdep.outputFile="$f" dependency:build-classpath
      [[ -s "$f" ]] || fail "missing classpath: $f"
    done
  done
fi
for f in "${classpath_files[@]}"; do
  [[ -s "$f" ]] || fail "missing classpath: $f"
  parts+=("$(cat "$f")")
done
CP="$(IFS=:; echo "${parts[*]}")"
check_class(){ local rel="$1" ok=1; IFS=: read -ra xs <<< "$CP"; for x in "${xs[@]}"; do if [[ -d "$x" && -f "$x/$rel" ]]; then ok=0; break; elif [[ -f "$x" && "$x" == *.jar ]] && jar tf "$x" 2>/dev/null | grep -Fxq "$rel"; then ok=0; break; fi; done; [[ $ok -eq 0 ]] || fail "required class missing: $rel"; }
CLASSPATH_VALIDATION_STAMP="$BUILD/classpath-validation-revision.txt"
if (( need_classpaths )) || [[ ! -f "$CLASSPATH_VALIDATION_STAMP" || \
      "$(cat "$CLASSPATH_VALIDATION_STAMP")" != "$actual" ]]; then
  check_class com/yahoo/search/yql/YqlParser.class
  check_class com/yahoo/language/opennlp/OpenNlpLinguistics.class
  check_class com/yahoo/prelude/IndexFacts.class
  printf '%s\n' "$actual" > "$CLASSPATH_VALIDATION_STAMP"
fi
rm -rf "$BUILD/classes"; mkdir -p "$BUILD/classes"
javac -Xlint:all,-path -Werror -cp "$CP" -d "$BUILD/classes" "$ORACLE_SOURCE"
printf '%s\n' "$CP" > "$BUILD/runtime-classpath.txt"
