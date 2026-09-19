use std::{env, error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let proto_root = manifest_dir.join("../api");
    let proto = proto_root.join("rpc/searchkernel.proto");

    println!("cargo:rerun-if-changed={}", proto.display());

    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    env::set_var("PROTOC", protoc);

    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile_protos(&[proto], &[proto_root])?;

    Ok(())
}
