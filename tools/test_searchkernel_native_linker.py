#!/usr/bin/env python3
from __future__ import annotations
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BUILD_RS = ROOT / "core/crates/searchkernel-vespa-bindings/build.rs"
BUILD_SDK = ROOT / "core/native/vespa/build-sdk.sh"
CONFIG = ROOT / ".cargo" / "config.toml"

class NativeRuntimeLinkTests(unittest.TestCase):
    def test_repository_does_not_require_a_custom_cargo_linker(self) -> None:
        config = CONFIG.read_text(encoding="utf-8")
        self.assertNotIn("searchkernel-native-linker", config)
        self.assertNotIn("[target.x86_64-unknown-linux-gnu]", config)

    def test_adapter_uses_origin_qualified_soname(self) -> None:
        source = BUILD_RS.read_text(encoding="utf-8")
        self.assertIn('$ORIGIN/.searchkernel-native/libsearchkernel_vespa.so.2', source)
        sdk = BUILD_SDK.read_text(encoding="utf-8")
        self.assertIn("SK_NATIVE_ADAPTER_SONAME", sdk)
        self.assertIn("-Wl,-soname,'$ADAPTER_SONAME'", sdk)

    def test_build_script_stages_runtime_for_cargo_executable_origins(self) -> None:
        source = BUILD_RS.read_text(encoding="utf-8")
        self.assertIn('profile_dir.join("deps")', source)
        self.assertIn('profile_dir.join("examples")', source)
        self.assertIn('stage_adapter_for_cargo_outputs(&out_dir, &adapter)', source)

    def test_sdk_builder_serializes_shared_target_publication(self) -> None:
        sdk = BUILD_SDK.read_text(encoding="utf-8")
        lock = sdk.index('SDK_LOCK="${SDK_PACKAGE}.build.lock"')
        provenance_guard = sdk.index("# Never let a full/custom SDK rebuild")
        staging = sdk.index('mktemp -d "${SDK_PACKAGE}.staging.XXXXXX"')
        self.assertLess(lock, provenance_guard)
        self.assertLess(provenance_guard, staging)
        self.assertIn('SK_NATIVE_SDK_PACKAGE_LOCK_HELD="$SDK_PACKAGE"', sdk)
        self.assertIn('exec flock "$SDK_LOCK" bash "$0" "$@"', sdk)
        self.assertNotIn('exec 9>"$SDK_LOCK"', sdk)
        self.assertNotIn("staging.$$", sdk)
        self.assertEqual(sdk.count('mktemp -d "${SDK_PACKAGE}.old.XXXXXX"'), 2)
        self.assertNotIn('SDK_BACKUP="${SDK_PACKAGE}.old.$"', sdk)
        self.assertNotIn('SDK_BACKUP="${SDK_PACKAGE}.old.$$"', sdk)

    def test_origin_needed_entry_resolves_without_loader_environment(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime = root / "target" / "debug" / ".searchkernel-native"
            runtime.mkdir(parents=True)
            lib = runtime / "libsearchkernel_vespa.so.2"
            lib_src = root / "adapter.c"
            lib_src.write_text("int sk_runtime_probe(void) { return 42; }\n")
            subprocess.run(["cc", "-shared", "-fPIC", "-Wl,-soname,$ORIGIN/.searchkernel-native/libsearchkernel_vespa.so.2", str(lib_src), "-o", str(lib)], check=True)
            (runtime / "libsearchkernel_vespa.so").symlink_to(lib.name)
            main = root / "main.c"
            main.write_text("extern int sk_runtime_probe(void); int main(void) { return sk_runtime_probe() == 42 ? 0 : 1; }\n")
            binary = root / "target" / "debug" / "probe"
            subprocess.run(["cc", str(main), f"-L{runtime}", "-lsearchkernel_vespa", "-o", str(binary)], check=True)
            dynamic = subprocess.run(["readelf", "-d", str(binary)], text=True, stdout=subprocess.PIPE, check=True).stdout
            self.assertIn("$ORIGIN/.searchkernel-native/libsearchkernel_vespa.so.2", dynamic)
            env = os.environ.copy(); env.pop("LD_LIBRARY_PATH", None)
            ran = subprocess.run([str(binary)], cwd="/", env=env, check=False)
            self.assertEqual(ran.returncode, 0)

if __name__ == "__main__":
    unittest.main()
