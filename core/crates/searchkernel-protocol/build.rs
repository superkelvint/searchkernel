use std::{env, path::PathBuf};

fn flatbuffers_root() -> PathBuf {
    PathBuf::from(env::var_os("FLATBUFFERS_CPP_ROOT").unwrap_or_else(|| "/usr/local".into()))
}

fn main() {
    println!("cargo:rerun-if-changed=native/flatbuffers_json_bridge.cc");
    println!("cargo:rerun-if-changed=native/flatbuffers_json_bridge.h");
    println!("cargo:rerun-if-changed=bfbs/searchkernel_request.bfbs");
    println!("cargo:rerun-if-changed=bfbs/searchkernel_response.bfbs");
    println!("cargo:rerun-if-env-changed=FLATBUFFERS_CPP_ROOT");

    let root = flatbuffers_root();
    let include = root.join("include");
    let library = root.join("lib");
    if !include.join("flatbuffers/idl.h").is_file() {
        panic!(
            "FlatBuffers 25.12.19 C++ headers were not found under '{}'; install the pinned FlatBuffers build under /usr/local or set FLATBUFFERS_CPP_ROOT",
            include.display()
        );
    }
    if !library.join("libflatbuffers.a").is_file() {
        panic!(
            "FlatBuffers 25.12.19 C++ library was not found at '{}'; install the pinned FlatBuffers build under /usr/local or set FLATBUFFERS_CPP_ROOT",
            library.join("libflatbuffers.a").display()
        );
    }

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
