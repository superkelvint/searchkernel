#!/usr/bin/env python3
from pathlib import Path
import json, sys
ROOT = Path(__file__).resolve().parents[1]
authority = json.loads((ROOT / "docs/release-support.json").read_text())
if authority.get("platform") != {"os":"linux","arch":"x86_64","libc":"glibc"}:
    raise SystemExit("release platform must remain explicit Linux x86_64 glibc")
expected = {
 ("rust-embedded","native","native-full"), ("searchkerneld","native","client-e2e"),
 ("python","managed-local","native-full"), ("python","connect","client-e2e"),
 ("typescript-node","connect","client-e2e"),
}
actual={(x["surface"],x["runtime"],x["evidence"]) for x in authority.get("supported",[])}
if actual != expected: raise SystemExit(f"supported release matrix drifted: {sorted(actual)}")
deferred=set(authority.get("deferred",[]))
required_deferred={"typescript-browser-grpc-web","php","ruby","go","macos","windows","linux-aarch64"}
if not required_deferred <= deferred: raise SystemExit("unsupported/deferred release surfaces are incomplete")
runner=(ROOT/".github/workflows/ci-image-runner.yml").read_text()
required_verification = {
 ("rust-embedded","native"): "./dev verify persistence",
 ("searchkerneld","native"): "tools/check-client-contracts.py --run-e2e",
 ("python","managed-local"): "tools/run-python-native-e2e.sh --prevalidated-native-sdk",
 ("python","connect"): "tools/check-client-contracts.py --run-e2e",
 ("typescript-node","connect"): "tools/check-client-contracts.py --run-e2e",
}
for row in authority["supported"]:
    key=(row["surface"],row["runtime"])
    if row.get("qualification") != "tools/qualify-consumer-release.py dist/release":
        raise SystemExit(f"{key} is missing packaged clean-machine qualification ownership")
    command=row.get("verification")
    if command != required_verification[key]: raise SystemExit(f"{key} has no authoritative executable verification command")
    lane=row["evidence"]
    marker=f"            {lane})"
    start=runner.find(marker)
    if start < 0: raise SystemExit(f"missing executable release lane: {lane}")
    end=runner.find("              ;;", start)
    if command not in runner[start:end]: raise SystemExit(f"release lane {lane} does not execute {key}: {command}")
readme=(ROOT/"README.md").read_text()
clients=(ROOT/"clients/README.md").read_text()
for text,name in [(readme,"README.md"),(clients,"clients/README.md")]:
    if "docs/release-support.md" not in text and "../docs/release-support.md" not in text:
        raise SystemExit(f"{name} does not link the authoritative release support matrix")
client_workflow = (ROOT / ".github/workflows/client-capabilities.yml").read_text()
for trigger in (
    "tools/package-consumer-release.py",
    "tools/check-consumer-release.py",
    "tools/check-release-support.py",
    "tools/qualify-consumer-release.py",
    "tools/test_package_consumer_release.py",
    "tools/test_qualify_consumer_release.py",
    "tools/bundle-native-runtime-closure.py",
    "tools/check-native-runtime-closure.py",
    "Cargo.toml",
    "Cargo.lock",
    "core/crates/searchkernel-vespa-bindings/**",
    "core/native/vespa/**",
    "searchkerneld/**",
    "reference/vespa/PINNED_VESPA.json",
    "docs/release-support.json",
    ".github/workflows/ci-image-runner.yml",
):
    if client_workflow.count(f'- "{trigger}"') < 2:
        raise SystemExit(
            f"release qualification owner does not trigger on PR and main changes to {trigger}"
        )
if "target: client-e2e" not in client_workflow:
    raise SystemExit("client capability workflow does not own executable client-e2e qualification")

consumer_marker = "- name: Qualify packaged release in clean consumer"
if consumer_marker not in runner:
    raise SystemExit("client-e2e lane has no isolated packaged consumer qualification")
consumer_step = runner.split(consumer_marker, 1)[1].split("\n      - name:", 1)[0]
for required in (
    '-v "$ARTIFACT_DIR/consumer-release:/release:ro"',
    '-v "$ARTIFACT_DIR/consumer-release-tools:/qualification:ro"',
    '"$PYTHON" /qualification/qualify-consumer-release.py /release',
):
    if required not in consumer_step:
        raise SystemExit(f"clean consumer qualification is missing: {required}")
for forbidden in (
    "$SOURCE_VOLUME",
    "$TARGET_VOLUME",
    "$NATIVE_VOLUME",
    "/work",
    "/cargo-target",
    "/opt/searchkernel/native-sdk-cache",
):
    if forbidden in consumer_step:
        raise SystemExit(
            f"clean consumer qualification leaks producer state: {forbidden}"
        )
print("Release support matrix PASS: Linux x86_64 glibc Rust/daemon/Python/Node scope is explicit and qualified")
