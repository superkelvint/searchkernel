#!/usr/bin/env python3
"""Regression coverage for the M11 verifier's native runtime ownership."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


COMMON = Path(__file__).with_name("verify-m11-common.sh")


class M11VerifierRuntimeTests(unittest.TestCase):
    def test_driver_build_uses_canonical_base_resolver_not_adapter_only_sdk(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw) / "repo"
            m11 = root / "reference/vespa/m11"
            common = root / "reference/vespa/common/verify-common.sh"
            tools = root / "tools/bin"
            sdk = root / "target/vespa-verifier/m11/sdk/hardening"
            target = root / "cargo-target"
            m11.mkdir(parents=True)
            common.parent.mkdir(parents=True)
            tools.mkdir(parents=True)
            (sdk / "lib").mkdir(parents=True)
            (sdk / "lib/libsearchkernel_vespa.so.2").write_text("sanitizer-adapter")
            (sdk / "share/embedded_crud_cfg").mkdir(parents=True)
            (m11 / "verify-m11-common.sh").write_text(COMMON.read_text())
            common.write_text(
                "require_native_driver() {\n"
                "  case \"$*\" in *sk_engine_group_tree*sk_generic_search_result_match_feature_value_ref*sk_test_failpoint_set*sk_test_failpoint_is_blocked*) ;; *) return 45 ;; esac\n"
                "}\n"
                "fail_verification() { echo \"verification error: $*\" >&2; return 1; }\n"
            )
            cargo = tools / "cargo"
            cargo.write_text(
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                "[[ -z \"${SK_NATIVE_SDK:-}\" ]] || { echo explicit-sdk-leaked >&2; exit 41; }\n"
                "[[ -z \"${SK_NATIVE_BASE_SDK:-}\" ]] || { echo base-sdk-leaked >&2; exit 42; }\n"
                "case \"${RUSTFLAGS:-}\" in *target/vespa-verifier/m11/sdk/hardening/lib*) ;; *) echo hardening-rpath-missing >&2; exit 46 ;; esac\n"
                "mkdir -p \"$CARGO_TARGET_DIR/debug/examples/.searchkernel-native\"\n"
                "printf normal-adapter > \"$CARGO_TARGET_DIR/debug/examples/.searchkernel-native/libsearchkernel_vespa.so.2\"\n"
                "cat > \"$CARGO_TARGET_DIR/debug/examples/searchkernel-m11-driver\" <<'DRIVER'\n"
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                "case \"${LD_LIBRARY_PATH:-}\" in *target/vespa-verifier/m11/sdk/hardening/lib*) ;; *) exit 43 ;; esac\n"
                "[[ \"${1:-}\" == compile ]] || exit 44\n"
                "mkdir -p \"${2:?}\"\n"
                "DRIVER\n"
                "chmod +x \"$CARGO_TARGET_DIR/debug/examples/searchkernel-m11-driver\"\n"
            )
            cargo.chmod(0o755)
            env = os.environ.copy()
            env.update(
                {
                    "PATH": f"{tools}:{env['PATH']}",
                    "CARGO_TARGET_DIR": str(target),
                    "SK_NATIVE_SDK": str(root / "invalid-explicit-adapter-sdk"),
                    "SK_NATIVE_BASE_SDK": str(root / "invalid-base-sdk"),
                }
            )
            command = (
                f"source {m11 / 'verify-m11-common.sh'}; "
                "build_m11_driver"
            )
            result = subprocess.run(
                ["bash", "-c", command],
                cwd=root,
                env=env,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertTrue((target / "debug/examples/searchkernel-m11-driver").is_file())
            staged = target / "debug/examples/.searchkernel-native/libsearchkernel_vespa.so.2"
            self.assertTrue(staged.is_file())
            self.assertFalse(staged.is_symlink())
            self.assertEqual(staged.read_bytes(), (sdk / "lib/libsearchkernel_vespa.so.2").read_bytes())

    def test_require_sdk_manifest_validates_manifest_owning_sdk(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw) / "repo"
            common_dir = root / "reference/vespa/common"
            source_header = root / "core/native/vespa/searchkernel_vespa.h"
            tools_dir = root / "tools"
            global_sdk = root / "cargo-target/native-probe-sdk"
            hardening_sdk = root / "target/vespa-verifier/m11/sdk/hardening"

            common_dir.mkdir(parents=True)
            source_header.parent.mkdir(parents=True)
            tools_dir.mkdir(parents=True)
            global_sdk.mkdir(parents=True)
            (hardening_sdk / "include").mkdir(parents=True)
            (hardening_sdk / "lib").mkdir(parents=True)

            common_source = (
                Path(__file__).parents[1] / "common" / "verify-common.sh"
            ).read_text()
            (common_dir / "verify-common.sh").write_text(common_source)

            header = "#define SK_NATIVE_ABI_VERSION 16U\n"
            source_header.write_text(header)
            (hardening_sdk / "include/searchkernel_vespa.h").write_text(header)
            (hardening_sdk / "manifest.json").write_text("{}\n")
            (tools_dir / "check-native-abi-authorities.py").write_text(
                "import sys\n"
                "raise SystemExit(0)\n"
            )

            env = os.environ.copy()
            env.update(
                {
                    "SK_NATIVE_SDK": str(global_sdk),
                    "VESPA_ROOT": str(root / "vespa"),
                }
            )
            result = subprocess.run(
                [
                    "bash",
                    "-c",
                    (
                        f"source {common_dir / 'verify-common.sh'}; "
                        f"require_sdk_manifest {hardening_sdk / 'manifest.json'}"
                    ),
                ],
                cwd=root,
                env=env,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_tensor_publication_config_stays_outside_canonical_config(self):
        driver = (
            Path(__file__).parents[2]
            / "core/crates/searchkernel/examples/searchkernel-m11-driver.rs"
        ).read_text()
        self.assertNotIn(
            'config.join("publication-tensor-config")',
            driver,
        )
        self.assertIn(
            '.join("publication-tensor-config")',
            driver,
        )
        tensor_setup = driver.split(
            "fn run_tensor_publication_regression", 1
        )[1].split("let tensor_type", 1)[0]
        self.assertIn("data\n        .parent()", tensor_setup)

    def test_sanitizer_verifier_repoints_driver_before_resolver_check(self):
        sanitizer = Path(__file__).with_name("verify-m11-sanitizers.sh").read_text()
        stage = sanitizer.index('stage_m11_driver_adapter "$binary" "$sdk"')
        resolve = sanitizer.index('resolved_adapter=$(LD_LIBRARY_PATH=')
        self.assertLess(stage, resolve)
        self.assertIn("m11_adapter_path_from_ldd", sanitizer)
        self.assertIn('cmp -s "$resolved_adapter" "$sdk/lib/libsearchkernel_vespa.so.2"', sanitizer)

    def test_ldd_parser_accepts_origin_and_legacy_shapes(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw) / "repo"
            m11 = root / "reference/vespa/m11"
            common = root / "reference/vespa/common/verify-common.sh"
            m11.mkdir(parents=True)
            common.parent.mkdir(parents=True)
            (m11 / "verify-m11-common.sh").write_text(COMMON.read_text())
            common.write_text(
                'fail_verification() { echo "verification error: $*" >&2; return 1; }\n'
            )
            env = os.environ.copy()
            env["CARGO_TARGET_DIR"] = str(root / "cargo-target")
            result = subprocess.run(
                [
                    "bash",
                    "-c",
                    f"source {m11 / 'verify-m11-common.sh'}; m11_adapter_path_from_ldd",
                ],
                cwd=root,
                env=env,
                text=True,
                input=(
                    "/tmp/debug/.searchkernel-native/libsearchkernel_vespa.so.2 (0x1234)\n"
                    "libsearchkernel_vespa.so.2 => /tmp/sdk/lib/libsearchkernel_vespa.so.2 (0x5678)\n"
                ),
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(
                result.stdout.strip(),
                "/tmp/debug/.searchkernel-native/libsearchkernel_vespa.so.2",
            )



if __name__ == "__main__":
    unittest.main()
