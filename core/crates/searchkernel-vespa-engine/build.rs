use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

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
    println!(
        "cargo:rerun-if-changed={}",
        sdk.join("native-build-fingerprint-v2.txt").display()
    );

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
    if let Some(out_dir) = env::var_os("OUT_DIR") {
        let out_dir = PathBuf::from(out_dir);
        if let Some(target_root) = out_dir.ancestors().nth(4) {
            return target_root.join("native-sdk/linux-x86_64");
        }
    }
    manifest_dir.join("../../native/sdk/linux-x86_64")
}
