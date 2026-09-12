use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let default_sdk = manifest_dir.join("../../native/sdk/linux-x86_64");
    let sdk = env::var_os("SK_NATIVE_SDK")
        .map(PathBuf::from)
        .unwrap_or(default_sdk);
    let lib_dir = sdk.join("lib");
    let header = sdk.join("include/searchkernel_vespa.h");
    let rust_abi =
        fs::read_to_string(manifest_dir.join("src/abi.rs")).expect("read Rust native ABI version");
    let expected_abi = rust_abi
        .lines()
        .find_map(|line| line.strip_prefix("pub const EXPECTED_ABI: u32 = "))
        .and_then(|value| value.strip_suffix(';'))
        .and_then(|value| value.parse::<u32>().ok())
        .expect("Rust ABI source must define EXPECTED_ABI");
    let header_text = fs::read_to_string(&header).expect("read native ABI header");
    let abi_version = header_text
        .lines()
        .find_map(|line| line.strip_prefix("#define SK_NATIVE_ABI_VERSION "))
        .and_then(|value| value.strip_suffix('U'))
        .and_then(|value| value.parse::<u32>().ok())
        .expect("native ABI header must define SK_NATIVE_ABI_VERSION");
    if abi_version != expected_abi {
        panic!(
            "native ABI header version {abi_version} does not match Rust crate ABI version {expected_abi}"
        );
    }

    if let Some(output) = env::var_os("SK_NATIVE_BINDINGS_OUTPUT") {
        let bindings = bindgen::Builder::default()
            .header(header.to_string_lossy())
            .allowlist_type("sk_(engine|document|document_update|search_request|search_result|text_document|text_update|text_request|text_result|text_occurrence|text_query_term|error)")
            .allowlist_function("sk_.*")
            .allowlist_var("SK_(NATIVE_ABI_VERSION|STATUS_.*)")
            .layout_tests(false)
            .generate_comments(false)
            .generate()
            .expect("generate native ABI bindings");
        bindings
            .write_to_file(output)
            .expect("write native ABI bindings");
    }

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=searchkernel_vespa");
    // Make binaries using the sys crate runnable without requiring callers to
    // arrange LD_LIBRARY_PATH themselves. The SDK path is already the path
    // selected for linking, so it is the correct runtime search location too.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!("cargo:rerun-if-env-changed=SK_NATIVE_SDK");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_BINDINGS_OUTPUT");
    println!("cargo:rerun-if-changed=src/abi.rs");
    println!("cargo:rerun-if-changed={}", header.display());
    println!("cargo:rerun-if-changed=abi_layout_check.c");
    cc::Build::new()
        .file(manifest_dir.join("abi_layout_check.c"))
        .include(sdk.join("include"))
        .flag_if_supported("-std=c11")
        .warnings(true)
        .compile("searchkernel_vespa_abi_layout");
}
