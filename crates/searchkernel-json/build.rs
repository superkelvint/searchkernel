use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let install_root = env::var_os("FLATBUFFERS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/local"));
    let include = env::var_os("FLATBUFFERS_INCLUDE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| install_root.join("include"));
    let lib = env::var_os("FLATBUFFERS_LIB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| install_root.join("lib"));

    let version_header = include.join("flatbuffers/base.h");
    let version = fs::read_to_string(&version_header).unwrap_or_else(|error| {
        panic!(
            "FlatBuffers 25.12.19 is required; could not read {}: {error}. Install the official C++ release and set FLATBUFFERS_ROOT, FLATBUFFERS_INCLUDE_DIR, and FLATBUFFERS_LIB_DIR as needed.",
            version_header.display()
        )
    });
    for (name, expected) in [
        ("FLATBUFFERS_VERSION_MAJOR", "25"),
        ("FLATBUFFERS_VERSION_MINOR", "12"),
        ("FLATBUFFERS_VERSION_REVISION", "19"),
    ] {
        let marker = format!("#define {name} {expected}");
        if !version.lines().any(|line| line.trim() == marker) {
            panic!(
                "FlatBuffers 25.12.19 is required, but {} does not define {}; install the matching official C++ release",
                version_header.display(),
                marker
            );
        }
    }

    let mut build = cc::Build::new();
    build.cpp(true).std("c++17").include(&include);

    let bridge = manifest.join("native/flatbuffers_json_bridge.cc");
    println!("cargo:rerun-if-changed={}", bridge.display());
    build.file(bridge);
    println!("cargo:rerun-if-changed=native/flatbuffers_json_bridge.h");
    build.compile("searchkernel_flatbuffers_json");
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=flatbuffers");
    println!("cargo:rerun-if-env-changed=FLATBUFFERS_ROOT");
    println!("cargo:rerun-if-env-changed=FLATBUFFERS_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=FLATBUFFERS_LIB_DIR");
}
