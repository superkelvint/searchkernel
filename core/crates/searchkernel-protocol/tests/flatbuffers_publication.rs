#![cfg(unix)]

#[path = "../build_support.rs"]
mod build_support;

#[test]
fn reader_keeps_its_generation_after_atomic_pointer_change() {
    let root = std::env::temp_dir().join(format!("searchkernel-flatbuffers-pin-{}", std::process::id()));
    std::fs::create_dir_all(root.join("old")).unwrap();
    std::fs::create_dir_all(root.join("new")).unwrap();
    let prefix = root.join("prefix");
    std::fs::create_dir_all(&prefix).unwrap();
    let current = prefix.join(".searchkernel-current");
    std::os::unix::fs::symlink(root.join("old"), &current).unwrap();
    let pinned = build_support::pin_flatbuffers_root(&prefix).unwrap();
    let replacement = prefix.join("next");
    std::os::unix::fs::symlink(root.join("new"), &replacement).unwrap();
    std::fs::rename(replacement, &current).unwrap();
    assert_eq!(pinned, root.join("old").canonicalize().unwrap());
    assert_eq!(
        build_support::pin_flatbuffers_root(&prefix).unwrap(),
        root.join("new").canonicalize().unwrap(),
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn dangling_publication_pointer_is_not_a_legacy_fallback() {
    let root = std::env::temp_dir().join(format!("searchkernel-flatbuffers-dangling-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::os::unix::fs::symlink(root.join("missing"), root.join(".searchkernel-current")).unwrap();
    assert!(build_support::pin_flatbuffers_root(&root).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
