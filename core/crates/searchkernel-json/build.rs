use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn flatbuffers_root() -> PathBuf {
    if let Ok(root) = env::var("FLATBUFFERS_CPP_ROOT") {
        return PathBuf::from(root);
    }

    if let Ok(flatc) = env::var("FLATC_PATH") {
        if let Some(root) = Path::new(&flatc).parent().and_then(Path::parent) {
            return root.to_path_buf();
        }
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let build_dir = out_dir
        .parent()
        .and_then(Path::parent)
        .expect("searchkernel-json OUT_DIR has the standard Cargo layout");
    let candidate = fs::read_dir(build_dir)
        .expect("read Cargo build directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("out"))
        .find(|out| out.join("bin/flatc").is_file() && out.join("include").is_dir());
    candidate.expect(
        "could not find vendored FlatBuffers C++ output; set FLATBUFFERS_CPP_ROOT to an official FlatBuffers build",
    )
}

fn main() {
    println!("cargo:rerun-if-changed=native/flatbuffers_json_bridge.cc");
    println!("cargo:rerun-if-changed=native/flatbuffers_json_bridge.h");
    println!("cargo:rerun-if-changed=bfbs/searchkernel_request.bfbs");
    println!("cargo:rerun-if-changed=bfbs/searchkernel_response.bfbs");
    println!("cargo:rerun-if-env-changed=FLATBUFFERS_CPP_ROOT");
    println!("cargo:rerun-if-env-changed=FLATC_PATH");

    let root = flatbuffers_root();
    let include = root.join("include");
    let library = root.join("lib");

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
