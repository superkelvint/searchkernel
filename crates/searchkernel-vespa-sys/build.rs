use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let default_sdk = manifest_dir.join("../../native/sdk/linux-x86_64");
    let sdk = env::var_os("SK_NATIVE_SDK")
        .map(PathBuf::from)
        .unwrap_or(default_sdk);
    let sdk = fs::canonicalize(&sdk)
        .unwrap_or_else(|error| panic!("canonicalize native SDK '{}': {error}", sdk.display()));
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

    let bindings_check = env::var_os("SK_NATIVE_BINDINGS_CHECK").is_some();
    if let Some(output) = env::var_os("SK_NATIVE_BINDINGS_OUTPUT").or_else(|| {
        bindings_check.then(|| {
            PathBuf::from(env::var_os("OUT_DIR").expect("build output directory"))
                .join("native_bindings.rs")
                .into_os_string()
        })
    }) {
        let bindings = generate_bindings(&header);
        bindings
            .write_to_file(&output)
            .expect("write native ABI bindings");
        if bindings_check {
            let checked_in = fs::read(manifest_dir.join("src/bindings.rs"))
                .expect("read checked-in native ABI bindings");
            let generated = fs::read(&output).expect("read generated native ABI bindings");
            if generated != checked_in {
                panic!(
                    "checked-in native ABI bindings are stale; regenerate them from {}",
                    header.display()
                );
            }
        }
    }

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=searchkernel_vespa");
    // Make binaries using the sys crate runnable without requiring callers to
    // arrange LD_LIBRARY_PATH themselves. The SDK path is already the path
    // selected for linking, so it is the correct runtime search location too.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!("cargo:rerun-if-env-changed=SK_NATIVE_SDK");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_BINDINGS_OUTPUT");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_BINDINGS_CHECK");
    println!("cargo:rerun-if-changed=src/abi.rs");
    println!("cargo:rerun-if-changed=src/bindings.rs");
    println!("cargo:rerun-if-changed={}", header.display());
    println!("cargo:rerun-if-changed=schema_identity_fd_test.cc");
    println!("cargo:rerun-if-changed=transport_lifecycle_test.cc");
    println!("cargo:rerun-if-changed=query_lifetime_test.cc");
    println!("cargo:rerun-if-changed=abi_noexcept_check.cc");
    println!("cargo:rerun-if-changed=native_header_namespace_check.cc");
    println!("cargo:rerun-if-changed=../../native/vespa/file_descriptor.h");
    println!("cargo:rerun-if-changed=../../native/vespa/transport_owner.h");
    cc::Build::new()
        .cpp(true)
        .file(manifest_dir.join("schema_identity_fd_test.cc"))
        .include(manifest_dir.join("../../native/vespa"))
        .warnings(true)
        .compile("searchkernel_schema_identity_fd_test");
    cc::Build::new()
        .cpp(true)
        .file(manifest_dir.join("transport_lifecycle_test.cc"))
        .include(manifest_dir.join("../../native/vespa"))
        .warnings(true)
        .compile("searchkernel_transport_lifecycle_test");
    cc::Build::new()
        .cpp(true)
        .file(manifest_dir.join("query_lifetime_test.cc"))
        .include(sdk.join("include"))
        .warnings(true)
        .compile("searchkernel_query_lifetime_test");
    cc::Build::new()
        .cpp(true)
        .file(manifest_dir.join("abi_noexcept_check.cc"))
        .include(sdk.join("include"))
        .warnings(true)
        .compile("searchkernel_vespa_abi_noexcept");
    cc::Build::new()
        .cpp(true)
        .file(manifest_dir.join("native_header_namespace_check.cc"))
        .include(sdk.join("include"))
        .include(manifest_dir.join("../searchkernel-ffi/include"))
        .warnings(true)
        .compile("searchkernel_vespa_header_namespace");
}

fn generate_bindings(header: &Path) -> bindgen::Bindings {
    bindgen::Builder::default()
        .header(header.to_string_lossy())
        .allowlist_type(
            "sk_(engine|document|document_update|search_request|search_result|text_document|text_update|text_request|text_result|query)",
        )
        .allowlist_function("sk_.*")
        .allowlist_var(
            "SK_(NATIVE_ABI_VERSION|ERROR_MESSAGE_CAPACITY|NATIVE_STATUS_.*|TEXT_QUERY_(TERM|PHRASE)|QUERY_(NODE_(AND|OR|NOT|TRUE)|TEXT_(TERM|PHRASE)|BOUND_(UNBOUNDED|INCLUDED|EXCLUDED)|FIELD_(TEXT|INTEGER)))",
        )
        .layout_tests(true)
        .generate_comments(false)
        .generate()
        .expect("generate native ABI bindings")
}
