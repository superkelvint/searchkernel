#![cfg(all(feature = "native", target_os = "linux"))]

#[path = "../src/abi.rs"]
mod abi;
#[path = "../sdk_abi.rs"]
mod sdk_abi;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "searchkernel-sdk-abi-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(root.join("include")).unwrap();
        fs::create_dir_all(root.join("lib")).unwrap();
        Self(root)
    }

    fn manifest(&self, text: &str) -> Result<(), String> {
        fs::write(self.0.join("manifest.json"), text).unwrap();
        sdk_abi::verify_manifest(
            &self.0,
            &self.0,
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.."),
            abi::EXPECTED_ABI,
        )
    }

    fn library(&self, actual: u32) -> PathBuf {
        fs::write(
            self.0.join("include/searchkernel_vespa.h"),
            format!(
                "#include <cstdint>\n#define SK_NATIVE_ABI_VERSION {}U\nextern \"C\" uint32_t sk_native_abi_version();\n",
                abi::EXPECTED_ABI,
            ),
        )
        .unwrap();
        let implementation = self.0.join("abi.cc");
        fs::write(
            &implementation,
            format!("#include <cstdint>\nextern \"C\" uint32_t sk_native_abi_version() {{ return {actual}; }}\n"),
        )
        .unwrap();
        let compiler = std::env::var_os("CXX").unwrap_or_else(|| "c++".into());
        let status = Command::new(&compiler)
            .args([
                "-shared",
                "-fPIC",
                "-Wl,-soname,$ORIGIN/.searchkernel-native/libsearchkernel_vespa.so.2",
            ])
            .arg(&implementation)
            .arg("-o")
            .arg(self.0.join("lib/libsearchkernel_vespa.so.2"))
            .status()
            .unwrap();
        assert!(status.success());
        fs::copy(
            self.0.join("lib/libsearchkernel_vespa.so.2"),
            self.0.join("lib/libsearchkernel_vespa.so"),
        )
        .unwrap();
        let source = self.0.join("probe.cc");
        fs::write(&source, sdk_abi::probe_source(abi::EXPECTED_ABI)).unwrap();
        let probe = self.0.join("probe");
        let status = Command::new(compiler)
            .arg(&source)
            .arg("-I")
            .arg(self.0.join("include"))
            .arg("-L")
            .arg(self.0.join("lib"))
            .arg("-lsearchkernel_vespa")
            .arg("-o")
            .arg(&probe)
            .status()
            .unwrap();
        assert!(status.success());
        probe
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn linked_library_must_report_the_header_abi() {
    let fixture = Fixture::new();
    let wrong = fixture.library(abi::EXPECTED_ABI + 1);
    let error = sdk_abi::execute_probe(&fixture.0, &wrong).unwrap_err();
    assert!(error.contains("ABI mismatch"), "{error}");
    let correct = fixture.library(abi::EXPECTED_ABI);
    sdk_abi::execute_probe(&fixture.0, &correct).unwrap();
}

#[test]
fn probe_header_must_agree_with_the_rust_authority() {
    let fixture = Fixture::new();
    fixture.library(abi::EXPECTED_ABI);
    let header = fixture.0.join("include/searchkernel_vespa.h");
    let original = fs::read_to_string(&header).unwrap();
    fs::write(
        header,
        original.replace(
            &format!("#define SK_NATIVE_ABI_VERSION {}U", abi::EXPECTED_ABI),
            &format!("#define SK_NATIVE_ABI_VERSION {}U", abi::EXPECTED_ABI + 1),
        ),
    )
    .unwrap();
    let compiler = std::env::var_os("CXX").unwrap_or_else(|| "c++".into());
    let output = Command::new(compiler)
        .arg(fixture.0.join("probe.cc"))
        .arg("-I")
        .arg(fixture.0.join("include"))
        .arg("-c")
        .arg("-o")
        .arg(fixture.0.join("mismatch.o"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("header disagrees with Rust ABI"));
}

#[test]
fn manifest_must_unambiguously_agree_with_rust() {
    let fixture = Fixture::new();
    fixture
        .manifest(&format!("{{\"native_abi_version\":{}}}", abi::EXPECTED_ABI))
        .unwrap();
    for manifest in [
        "{}".to_owned(),
        "{\"native_abi_version\":true}".to_owned(),
        format!("{{\"native_abi_version\":{}}}", abi::EXPECTED_ABI + 1),
        format!(
            "{{\"native_abi_version\":{},\"native_abi_version\":{}}}",
            abi::EXPECTED_ABI,
            abi::EXPECTED_ABI
        ),
        format!("{{\"native_abi_version\":\"{}\"}}", abi::EXPECTED_ABI),
    ] {
        assert!(fixture.manifest(&manifest).is_err(), "{manifest}");
    }
}

#[test]
fn cross_compilation_cannot_claim_runtime_validation() {
    assert!(
        sdk_abi::require_native_host("aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu")
            .is_err()
    );
    sdk_abi::require_native_host("x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu").unwrap();
}
