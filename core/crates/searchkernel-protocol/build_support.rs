use std::path::{Path, PathBuf};

pub fn pin_flatbuffers_root(root: &Path) -> Result<PathBuf, String> {
    let current = root.join(".searchkernel-current");
    let selected = if current.symlink_metadata().is_ok() {
        if !current.is_symlink() {
            return Err(format!("invalid FlatBuffers publication pointer: {}", current.display()));
        }
        current
    } else {
        root.to_path_buf()
    };
    // Return an immutable generation, not the mutable selector. Subsequent
    // header reads, C++ compilation and Cargo linking use this same path.
    selected.canonicalize().map_err(|error| {
        format!("resolve FlatBuffers installation '{}': {error}", selected.display())
    })
}
