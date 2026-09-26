#!/usr/bin/env python3
from pathlib import Path
import json, sys
ROOT = Path(__file__).resolve().parents[1]
authority = json.loads((ROOT / "docs/release-support.json").read_text())
if authority.get("platform") != {"os":"linux","arch":"x86_64","libc":"glibc"}:
    raise SystemExit("release platform must remain explicit Linux x86_64 glibc")
expected = {
 ("rust-embedded","native","native-full"), ("searchkerneld","native","native-full"),
 ("python","managed-local","client-e2e"), ("python","connect","client-e2e"),
 ("typescript-node","managed-local","client-e2e"), ("typescript-node","connect","client-e2e"),
}
actual={(x["surface"],x["runtime"],x["evidence"]) for x in authority.get("supported",[])}
if actual != expected: raise SystemExit(f"supported release matrix drifted: {sorted(actual)}")
deferred=set(authority.get("deferred",[]))
required_deferred={"typescript-browser-grpc-web","php","ruby","go","macos","windows","linux-aarch64"}
if not required_deferred <= deferred: raise SystemExit("unsupported/deferred release surfaces are incomplete")
runner=(ROOT/".github/workflows/ci-image-runner.yml").read_text()
for lane in {x["evidence"] for x in authority["supported"]}:
    if f"            {lane})" not in runner: raise SystemExit(f"missing executable release lane: {lane}")
readme=(ROOT/"README.md").read_text()
clients=(ROOT/"clients/README.md").read_text()
for text,name in [(readme,"README.md"),(clients,"clients/README.md")]:
    if "docs/release-support.md" not in text and "../docs/release-support.md" not in text:
        raise SystemExit(f"{name} does not link the authoritative release support matrix")
print("Release support matrix PASS: Linux x86_64 glibc Rust/daemon/Python/Node scope is explicit and qualified")
