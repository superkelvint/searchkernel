use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("build output directory"));
    let config = manifest_dir.join("cbindgen.toml");
    let generated_path = output_dir.join("searchkernel_ffi.h");

    let generated = cbindgen::Builder::new()
        .with_crate(&manifest_dir)
        .with_config(cbindgen::Config::from_file(&config).expect("read cbindgen config"))
        .generate()
        .expect("generate Search Kernel FFI header");
    generated.write_to_file(&generated_path);

    let checked_in = manifest_dir.join("include/searchkernel_ffi.h");
    let generated_bytes = fs::read(&generated_path).expect("read generated FFI header");
    let checked_in_bytes = fs::read(&checked_in).expect("read checked-in FFI header");
    if generated_bytes != checked_in_bytes {
        panic!(
            "checked-in Search Kernel FFI header is stale; regenerate it with tools/generate-searchkernel-ffi-header.sh"
        );
    }

    println!("cargo:rerun-if-changed={}", config.display());
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("src/lib.rs").display()
    );
}
