use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(searchkernel_hardening_sdk)");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_SDK");

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let sdk = env::var_os("SK_NATIVE_SDK")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../native/sdk/linux-x86_64"));
    let manifest = sdk.join("manifest.json");
    println!("cargo:rerun-if-changed={}", manifest.display());

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
