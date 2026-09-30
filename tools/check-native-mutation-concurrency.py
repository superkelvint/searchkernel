#!/usr/bin/env python3
from pathlib import Path
root = Path(__file__).resolve().parents[1]
engine = (root / "core/native/vespa/engine.cc").read_text()
driver = (root / "core/crates/searchkernel/examples/searchkernel-m11-driver.rs").read_text()
adapter = (root / "core/crates/searchkernel-ir-adapter/src/backend.rs").read_text()
if "_mutation_mutex" in engine or "mutation_guard" in engine:
    raise SystemExit("native adapter still globally serializes operations")
if "existence_mutation_lock: Mutex<()>" in adapter:
    raise SystemExit("Rust adapter still has a process-wide existence mutation lock")
for token in ["existence_mutation_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>", "_bucket_mutex", "_next_timestamp", 'hardening::set(&engine, "mutation_after_native_call", 1, Action::Block)', "independent mutation was globally serialized"]:
    if token not in engine + driver + adapter:
        raise SystemExit(f"native concurrency contract missing: {token}")
print("Native mutation concurrency contract PASS")
