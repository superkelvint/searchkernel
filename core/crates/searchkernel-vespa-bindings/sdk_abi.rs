use std::fs;
use std::path::Path;
use std::process::Command;

const PROBE_SOURCE: &str = r#"#include <searchkernel_vespa.h>
#include <cstdio>
static_assert(SK_NATIVE_ABI_VERSION == SK_EXPECTED_ABI,
              "native SDK header disagrees with Rust ABI");
int main() {
    const auto actual = sk_native_abi_version();
    if (actual != SK_EXPECTED_ABI) {
        std::fprintf(stderr, "native SDK ABI mismatch: library=%u header=%u\n",
                     actual, static_cast<unsigned>(SK_EXPECTED_ABI));
        return 1;
    }
    return 0;
}
"#;

pub fn probe_source(expected: u32) -> String {
    // A literal cannot be redefined by a consumer's compiler -D flags.
    PROBE_SOURCE.replace("SK_EXPECTED_ABI", &expected.to_string())
}

pub fn require_native_host(target: &str, host: &str) -> Result<(), String> {
    if target != host {
        return Err(format!(
            "native SDK runtime ABI validation requires target=host; cross-compilation from {host} to {target} is unsupported"
        ));
    }
    Ok(())
}

pub fn verify_manifest(
    sdk: &Path,
    work: &Path,
    support_root: &Path,
    expected: u32,
) -> Result<(), String> {
    let output = Command::new("bash")
        .arg(support_root.join("tools/resolve-python.sh"))
        .output()
        .map_err(|error| format!("resolve canonical Python for native SDK manifest: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "resolve canonical Python for native SDK manifest: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let python = String::from_utf8(output.stdout)
        .map_err(|error| format!("canonical Python path is not UTF-8: {error}"))?;
    let checker = work.join("sdk_manifest_abi.py");
    fs::write(&checker, include_str!("sdk_manifest_abi.py"))
        .map_err(|error| format!("materialize SDK manifest ABI checker: {error}"))?;
    let output = Command::new(python.trim())
        .arg(checker)
        .arg(sdk.join("manifest.json"))
        .arg(expected.to_string())
        .output()
        .map_err(|error| format!("validate native SDK manifest ABI: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "native SDK manifest ABI validation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

pub fn execute_probe(sdk: &Path, executable: &Path) -> Result<(), String> {
    // The adapter SONAME is $ORIGIN/.searchkernel-native/..., so an rpath
    // alone cannot make a freshly linked probe executable load it.
    let origin = executable
        .parent()
        .ok_or("native SDK probe has no parent")?;
    let runtime = origin.join(".searchkernel-native");
    fs::create_dir_all(&runtime).map_err(|error| format!("stage SDK ABI probe: {error}"))?;
    fs::copy(
        sdk.join("lib/libsearchkernel_vespa.so.2"),
        runtime.join("libsearchkernel_vespa.so.2"),
    )
    .map_err(|error| format!("stage SDK ABI probe adapter: {error}"))?;
    let output = Command::new(executable)
        .output()
        .map_err(|error| format!("execute native SDK ABI probe: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "native SDK ABI probe failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}
