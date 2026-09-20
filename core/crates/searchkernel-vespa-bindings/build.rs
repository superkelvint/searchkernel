use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    if env::var_os("CARGO_FEATURE_NATIVE").is_none() {
        // The workspace's no-default-features lane still builds this member,
        // but must not require the optional Vespa SDK just to compile the
        // checked-in ABI declarations.
        return;
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let default_sdk = manifest_dir.join("../../native/sdk/linux-x86_64");
    let sdk = env::var_os("SK_NATIVE_SDK")
        .map(PathBuf::from)
        .unwrap_or(default_sdk);
    let sdk = fs::canonicalize(&sdk)
        .unwrap_or_else(|error| panic!("canonicalize native SDK '{}': {error}", sdk.display()));
    let lib_dir = sdk.join("lib");
    let source_header = manifest_dir.join("../../native/vespa/searchkernel_vespa.h");
    let source_header = fs::canonicalize(&source_header).unwrap_or_else(|error| {
        panic!(
            "canonicalize source native ABI header '{}': {error}",
            source_header.display()
        )
    });
    let using_packaged_sdk =
        env::var_os("SK_NATIVE_SDK").is_none() && env::var_os("SK_NATIVE_ABI_HEADER").is_none();
    let header = env::var_os("SK_NATIVE_ABI_HEADER")
        .map(PathBuf::from)
        .unwrap_or_else(|| sdk.join("include/searchkernel_vespa.h"));
    if using_packaged_sdk {
        let source_bytes = fs::read(&source_header).unwrap_or_else(|error| {
            panic!(
                "read source native ABI header '{}': {error}",
                source_header.display()
            )
        });
        let sdk_bytes = fs::read(&header).unwrap_or_else(|error| {
            panic!(
                "read packaged native ABI header '{}': {error}",
                header.display()
            )
        });
        if source_bytes != sdk_bytes {
            panic!(
                "packaged native ABI header '{}' differs from source header '{}'; rebuild the SDK from the source header",
                header.display(),
                source_header.display()
            );
        }
    }

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

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=searchkernel_vespa");
    // Make binaries using the raw bindings crate runnable without requiring
    // callers to arrange LD_LIBRARY_PATH themselves. The SDK path is already
    // the path selected for linking, so it is the correct runtime search
    // location too.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!("cargo:rerun-if-env-changed=SK_NATIVE_SDK");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_ABI_HEADER");
    println!("cargo:rerun-if-changed={}", source_header.display());
    println!("cargo:rerun-if-env-changed=SK_NATIVE_ENABLE_FAILPOINTS");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_EXTRA_CXX_FLAGS");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_EXTRA_LINK_FLAGS");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_SANITIZER");
    println!("cargo:rerun-if-changed=src/abi.rs");
    println!("cargo:rerun-if-changed=src/bindings.rs");
    println!("cargo:rerun-if-changed={}", header.display());

    #[cfg(feature = "native-contract-tests")]
    compile_contract_tests(&manifest_dir, &sdk);
}

#[cfg(feature = "native-contract-tests")]
fn compile_contract_tests(manifest_dir: &Path, sdk: &Path) {
    let verifier_dir = manifest_dir.join("../../../tools/native-contract-tests");
    println!(
        "cargo:rerun-if-changed={}",
        verifier_dir.join("schema_identity_fd_test.cc").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        verifier_dir.join("transport_lifecycle_test.cc").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        verifier_dir.join("query_lifetime_test.cc").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        verifier_dir.join("abi_noexcept_check.cc").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        verifier_dir
            .join("native_header_namespace_check.cc")
            .display()
    );
    println!("cargo:rerun-if-changed=../../native/vespa/file_descriptor.h");
    println!("cargo:rerun-if-changed=../../native/vespa/transport_owner.h");
    cc::Build::new()
        .cpp(true)
        .file(verifier_dir.join("schema_identity_fd_test.cc"))
        .include(manifest_dir.join("../../native/vespa"))
        .warnings(true)
        .compile("searchkernel_schema_identity_fd_test");
    cc::Build::new()
        .cpp(true)
        .file(verifier_dir.join("transport_lifecycle_test.cc"))
        .include(manifest_dir.join("../../native/vespa"))
        .warnings(true)
        .compile("searchkernel_transport_lifecycle_test");
    cc::Build::new()
        .cpp(true)
        .file(verifier_dir.join("query_lifetime_test.cc"))
        .include(sdk.join("include"))
        .warnings(true)
        .compile("searchkernel_query_lifetime_test");
    cc::Build::new()
        .cpp(true)
        .file(verifier_dir.join("abi_noexcept_check.cc"))
        .include(sdk.join("include"))
        .warnings(true)
        .compile("searchkernel_vespa_abi_noexcept");
    cc::Build::new()
        .cpp(true)
        .file(verifier_dir.join("native_header_namespace_check.cc"))
        .include(sdk.join("include"))
        .warnings(true)
        .compile("searchkernel_vespa_header_namespace");
}
