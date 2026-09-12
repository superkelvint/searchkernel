fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc is available");
    std::env::set_var("PROTOC", protoc);

    prost_build::Config::new()
        .compile_protos(&["../../api/searchkernel.proto"], &["../../api"])
        .expect("searchkernel protobuf compiles");

    println!("cargo:rerun-if-changed=../../api/searchkernel.proto");
}
