use std::{env, fs, path::PathBuf};

const FLATBUFFERS_VERSION: &str = "25.12.19";

fn print_native_inputs() {
    println!("cargo:rerun-if-changed=native");
    println!("cargo:rerun-if-changed=bfbs/searchkernel_request.bfbs");
    println!("cargo:rerun-if-changed=bfbs/searchkernel_response.bfbs");
    println!("cargo:rerun-if-env-changed=DEP_FLATBUFFERS_BUILD_ROOT");

    for name in [
        "PATH",
        "CC",
        "CXX",
        "AR",
        "RANLIB",
        "CFLAGS",
        "CXXFLAGS",
        "CPPFLAGS",
        "LDFLAGS",
        "CXXSTDLIB",
        "CRATE_CC_NO_DEFAULTS",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }

    let target = env::var("TARGET").unwrap_or_default();
    let target_underscore = target.replace('-', "_");
    for base in [
        "CC", "CXX", "AR", "RANLIB", "CFLAGS", "CXXFLAGS", "CPPFLAGS", "LDFLAGS",
    ] {
        for name in [
            format!("{base}_{target}"),
            format!("{base}_{target_underscore}"),
            format!("HOST_{base}"),
        ] {
            println!("cargo:rerun-if-env-changed={name}");
        }
    }
}

fn main() {
    print_native_inputs();

    let root = PathBuf::from(
        env::var_os("DEP_FLATBUFFERS_BUILD_ROOT")
            .expect("flatbuffers-build did not export its native root"),
    );
    let include = root.join("include");
    let library = root.join("lib");
    verify_flatbuffers_version(&include)
        .expect("FlatBuffers headers do not match the vendored library");

    cc::Build::new()
        .cpp(true)
        .file("native/flatbuffers_json_bridge.cc")
        .include("native")
        .include(include)
        .flag_if_supported("-std=c++17")
        .warnings(true)
        .compile("searchkernel_flatbuffers_json_bridge");

    println!("cargo:rustc-link-search=native={}", library.display());
    println!("cargo:rustc-link-lib=static=flatbuffers");
    if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=dylib=c++");
    }
}

fn verify_flatbuffers_version(include: &std::path::Path) -> Result<(), String> {
    let header = fs::read_to_string(include.join("flatbuffers/base.h"))
        .map_err(|error| format!("read FlatBuffers version header: {error}"))?;
    let major = header
        .lines()
        .find_map(|line| line.strip_prefix("#define FLATBUFFERS_VERSION_MAJOR "))
        .ok_or_else(|| "FlatBuffers major version is missing".to_owned())?;
    let minor = header
        .lines()
        .find_map(|line| line.strip_prefix("#define FLATBUFFERS_VERSION_MINOR "))
        .ok_or_else(|| "FlatBuffers minor version is missing".to_owned())?;
    let revision = header
        .lines()
        .find_map(|line| line.strip_prefix("#define FLATBUFFERS_VERSION_REVISION "))
        .ok_or_else(|| "FlatBuffers revision is missing".to_owned())?;
    let actual = format!("{major}.{minor}.{revision}");
    if actual == FLATBUFFERS_VERSION {
        Ok(())
    } else {
        Err(format!("expected {FLATBUFFERS_VERSION}, got {actual}"))
    }
}
