use std::{env, error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let output = PathBuf::from(
        env::args_os()
            .nth(1)
            .ok_or("usage: searchkernel-rpc-codegen OUTPUT_DIRECTORY")?,
    );
    let tool_root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let repository_root = tool_root
        .parent()
        .and_then(|path| path.parent())
        .ok_or("codegen tool is not under the repository tools directory")?;
    let proto_root = repository_root.join("api");
    let proto = proto_root.join("rpc/searchkernel.proto");

    std::fs::create_dir_all(&output)?;
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    env::set_var("PROTOC", protoc);
    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .out_dir(&output)
        .compile_protos(&[proto], &[proto_root])?;
    Ok(())
}
