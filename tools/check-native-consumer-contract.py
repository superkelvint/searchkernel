#!/usr/bin/env python3
from pathlib import Path
import subprocess
ROOT = Path(__file__).resolve().parents[1]
subprocess.run(["python3", str(ROOT / "tools/generate-native-consumer-bundle.py"), "--check"], check=True)
build = (ROOT / "core/crates/searchkernel-vespa-bindings/build.rs").read_text()
resolver = (ROOT / "tools/resolve-native-sdk.sh").read_text()
installer = (ROOT / "tools/install-native-base-artifact.sh").read_text()
required_build = ['resolve_shared_native_sdk(&manifest_dir, &target, "ensure")', 'do not set SK_NATIVE_SDK', 'mod consumer_bundle;', 'support_root(&manifest_dir)', 'SEARCHKERNEL_NATIVE_CONSUMER_BUILD', 'if mode == "ensure" { "1" } else { "0" }']
for token in required_build:
    if token not in build: raise SystemExit(f"consumer native build contract missing: {token}")
if 'resolve_shared_native_sdk(&manifest_dir, &target, "path")' in build:
    raise SystemExit("consumer native build still requires manual prerequisite provisioning")
for token in ['published native-base cache is older than the desired generation; refresh required', 'quarantining and rebuilding under the checkout lock', 'install_published_base']:
    if token not in resolver: raise SystemExit(f"resolver lost automatic refresh/repair behavior: {token}")
if 'consumer native build refuses to build Vespa from source' not in resolver:
    raise SystemExit("consumer native build may fall back to rebuilding Vespa from source")
for token in ['published Vespa base checksum mismatch', 'native_target', 'vespa_commit']:
    if token not in installer: raise SystemExit(f"installer lost artifact validation: {token}")
runner = (ROOT / ".github/workflows/ci-image-runner.yml").read_text()
if "native-full-consumer-package bash tools/test-native-consumer-package.sh" not in runner:
    raise SystemExit("native-full lane does not exercise the packaged consumer outside the monorepo")
print("Native consumer contract PASS: native provisioning is automatic, validated, and self-repairing")
