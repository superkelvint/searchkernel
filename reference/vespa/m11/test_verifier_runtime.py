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
                "  case \"$*\" in *sk_test_failpoint_set*sk_test_failpoint_is_blocked*) ;; *) return 45 ;; esac\n"
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
                "mkdir -p \"$CARGO_TARGET_DIR/debug/.searchkernel-native\"\n"
                "printf normal-adapter > \"$CARGO_TARGET_DIR/debug/.searchkernel-native/libsearchkernel_vespa.so.2\"\n"
                "cat > \"$CARGO_TARGET_DIR/debug/searchkernel-m11-driver\" <<'DRIVER'\n"
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                "case \"${LD_LIBRARY_PATH:-}\" in *target/vespa-verifier/m11/sdk/hardening/lib*) ;; *) exit 43 ;; esac\n"
                "[[ \"${1:-}\" == compile ]] || exit 44\n"
                "mkdir -p \"${2:?}\"\n"
                "DRIVER\n"
                "chmod +x \"$CARGO_TARGET_DIR/debug/searchkernel-m11-driver\"\n"
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
            self.assertTrue((target / "debug/searchkernel-m11-driver").is_file())
            staged = target / "debug/.searchkernel-native/libsearchkernel_vespa.so.2"
            self.assertTrue(staged.is_symlink())
            self.assertEqual(staged.resolve(), (sdk / "lib/libsearchkernel_vespa.so.2").resolve())

    def test_sanitizer_verifier_repoints_driver_before_resolver_check(self):
        sanitizer = Path(__file__).with_name("verify-m11-sanitizers.sh").read_text()
        stage = sanitizer.index('stage_m11_driver_adapter "$binary" "$sdk"')
        resolve = sanitizer.index('resolved_adapter=$(LD_LIBRARY_PATH=')
        self.assertLess(stage, resolve)
        self.assertIn("for (index = 1; index <= NF; ++index)", sanitizer)


if __name__ == "__main__":
    unittest.main()
