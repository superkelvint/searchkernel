use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(searchkernel_hardening_sdk)");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_SDK");

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest_dir.join("../../..");
    let sdk = env::var_os("SK_NATIVE_SDK")
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        })
        .unwrap_or_else(|| default_native_sdk(&manifest_dir));
    let manifest = sdk.join("manifest.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let fingerprint = sdk.join("native-build-fingerprint-v2.txt");
    if fingerprint.is_file() {
        println!("cargo:rerun-if-changed={}", fingerprint.display());
    }

    let Ok(contents) = fs::read_to_string(&manifest) else {
        return;
    };
    let hardening_enabled = contents.lines().any(|line| {
        let Some((key, value)) = line.split_once(':') else {
            return false;
        };
        key.trim() == "\"hardening_failpoints\"" && value.trim().trim_end_matches(',') == "true"
    });

    if hardening_enabled {
        println!("cargo:rustc-cfg=searchkernel_hardening_sdk");
    }
}

fn default_native_sdk(manifest_dir: &Path) -> PathBuf {
    // Canonical/container builds mount the immutable native base outside the
    // writable Cargo target tree. Resolve that authority first so Cargo does
    // not fingerprint a permanently-missing target/native-sdk path and rebuild
    // every native dependent on each fresh container invocation.
    let root = manifest_dir.join("../../..");
    let resolver = root.join("tools/resolve-native-sdk.sh");
    if resolver.is_file() {
        let mut command = Command::new("bash");
        command.arg(&resolver).arg("path");
        if let Some(target) = env::var_os("TARGET") {
            command.env("SK_NATIVE_TARGET", target);
        }
        if let Ok(output) = command.output() {
            if output.status.success() {
                if let Some(path) = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                {
                    return PathBuf::from(path);
                }
            }
        }
    }
    if let Some(out_dir) = env::var_os("OUT_DIR") {
        let out_dir = PathBuf::from(out_dir);
        if let Some(target_root) = out_dir.ancestors().nth(4) {
            return target_root.join("native-sdk/linux-x86_64");
        }
    }
    manifest_dir.join("../../native/sdk/linux-x86_64")
}
