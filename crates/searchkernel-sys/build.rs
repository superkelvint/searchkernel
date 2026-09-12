use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let default_sdk = manifest_dir.join("../../native/sdk/linux-x86_64");
    let sdk = env::var_os("SK_NATIVE_SDK")
        .map(PathBuf::from)
        .unwrap_or(default_sdk);
    let lib_dir = sdk.join("lib");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=searchkernel_vespa");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_SDK");
    println!(
        "cargo:rerun-if-changed={}",
        sdk.join("include/searchkernel_vespa.h").display()
    );
}
