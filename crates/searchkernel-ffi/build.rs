use std::env;
use std::fs;
use std::path::PathBuf;

const STATUS_NAMES: &[&str] = &[
    "SK_STATUS_OK",
    "SK_STATUS_INVALID_ARGUMENT",
    "SK_STATUS_INVALID_HANDLE",
    "SK_STATUS_INVALID_REQUEST",
    "SK_STATUS_UNSUPPORTED_PROTOCOL",
    "SK_STATUS_UNSUPPORTED_OPERATION",
    "SK_STATUS_INTERNAL",
    "SK_STATUS_PANIC",
];

fn value_after(source: &str, name: &str) -> u32 {
    source
        .lines()
        .find_map(|line| {
            let line = line.trim();
            let value = line.strip_prefix(&format!("#define {name} "))?;
            parse_number(value)
        })
        .or_else(|| {
            source.lines().find_map(|line| {
                let line = line.trim();
                let (candidate, value) = line.split_once('=')?;
                if candidate.trim() != name {
                    return None;
                }
                parse_number(value)
            })
        })
        .unwrap_or_else(|| panic!("header does not define {}", name))
}

fn parse_number(value: &str) -> Option<u32> {
    value.trim().trim_end_matches([',', 'u', 'U']).parse().ok()
}

fn main() {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let header = manifest_dir.join("include/searchkernel_ffi.h");
    let source = fs::read_to_string(&header).expect("read public FFI header");
    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("build output directory"));

    let mut generated = format!(
        "pub(crate) const ABI_VERSION: u32 = {};\n",
        value_after(&source, "SK_ABI_VERSION")
    );
    for name in STATUS_NAMES {
        let rust_name = name.strip_prefix("SK_STATUS_").expect("status prefix");
        generated.push_str(&format!(
            "pub(crate) const STATUS_{rust_name}: u32 = {};\n",
            value_after(&source, name)
        ));
    }
    fs::write(output_dir.join("abi_contract.rs"), generated)
        .expect("write generated FFI ABI contract");

    println!("cargo:rerun-if-changed={}", header.display());
}
