#![cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]

#[path = "../consumer_layout.rs"]
mod consumer_layout;
#[path = "../consumer_runtime.rs"]
mod consumer_runtime;

use flate2::{write::GzEncoder, Compression};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::{Path, PathBuf}, process::Command, time::{SystemTime, UNIX_EPOCH}};

struct Fixture {
    root: PathBuf,
    identity: Value,
    descriptor: Value,
}

fn digest(root: &Path) -> String {
    fn files(root: &Path, current: &Path, items: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() { files(root, &path, items); }
            else if path.file_name().unwrap() != "manifest.json" { items.push(path.strip_prefix(root).unwrap().to_path_buf()); }
        }
    }
    let mut items = Vec::new();
    files(root, root, &mut items);
    items.sort();
    let mut digest = Sha256::new();
    for relative in items {
        let name = relative.to_str().unwrap().as_bytes();
        let data = fs::read(root.join(&relative)).unwrap();
        digest.update((name.len() as u64).to_be_bytes()); digest.update(name);
        digest.update((data.len() as u64).to_be_bytes()); digest.update(data);
    }
    format!("{:x}", digest.finalize())
}

impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("sk-cargo-runtime-{}-{stamp}", std::process::id()));
        fs::create_dir_all(root.join("runtime/bin")).unwrap();
        fs::create_dir_all(root.join("runtime/lib")).unwrap();
        let vectors: Value = serde_json::from_slice(&fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../build/consumer-runtime/conformance.json")
        ).unwrap()).unwrap();
        let vector = &vectors["vectors"][0];
        let mut identity = vector["identity_material"].clone();
        identity["format_version"] = json!(1);
        identity["runtime_identity"] = vector["runtime_identity"].clone();
        for (name, data) in [("bin/searchkerneld", "launcher"), ("bin/searchkerneld.real", "daemon"), ("lib/libsearchkernel_vespa.so.2", "adapter")] {
            let path = root.join("runtime").join(name);
            fs::write(&path, data).unwrap();
            #[cfg(unix)] {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        identity["payload_sha256"] = json!(digest(&root.join("runtime")));
        fs::write(root.join("runtime/manifest.json"), serde_json::to_vec(&identity).unwrap()).unwrap();
        let name = format!("searchkernel-runtime-{}-{}.tar.gz", identity["runtime_identity"].as_str().unwrap(), identity["platform"].as_str().unwrap());
        let archive_path = root.join(name);
        let mut archive = tar::Builder::new(GzEncoder::new(fs::File::create(&archive_path).unwrap(), Compression::default()));
        for name in ["bin", "lib"] { archive.append_dir_all(name, root.join("runtime").join(name)).unwrap(); }
        archive.append_path_with_name(root.join("runtime/manifest.json"), "manifest.json").unwrap();
        archive.into_inner().unwrap().finish().unwrap();
        let mut descriptor = identity.clone();
        descriptor["artifact_url"] = json!(url::Url::from_file_path(&archive_path).unwrap().to_string());
        descriptor["artifact_sha256"] = json!(format!("{:x}", Sha256::digest(fs::read(archive_path).unwrap())));
        let fixture = Self { root, identity, descriptor };
        fixture.write_descriptor();
        fixture
    }

    fn write_descriptor(&self) {
        fs::write(self.root.join("runtime.json"), serde_json::to_vec(&self.descriptor).unwrap()).unwrap();
    }

    fn worker(&self) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.arg("--exact").arg("runtime_worker").arg("--nocapture")
            .env("SK_RUNTIME_TEST_DESCRIPTOR", self.root.join("runtime.json"))
            .env("SEARCHKERNEL_RUNTIME_CACHE", self.root.join("cache"))
            .env_remove("SEARCHKERNEL_RUNTIME_DIR").env_remove("SEARCHKERNEL_RUNTIME_ORIGIN");
        command
    }

    fn object(&self) -> PathBuf { self.root.join("cache/objects").join(self.identity["runtime_identity"].as_str().unwrap()) }
}

impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); } }

// Process isolation is intentional: no test mutates another test's environment.
#[test]
fn runtime_worker() {
    let Some(path) = std::env::var_os("SK_RUNTIME_TEST_DESCRIPTOR") else { return; };
    let descriptor = consumer_runtime::load_descriptor(Path::new(&path)).unwrap();
    match consumer_runtime::ensure_runtime(&descriptor) {
        Ok(path) => println!("runtime={}", path.display()),
        Err(error) => panic!("{error}"),
    }
}

#[test]
fn shared_vector_cold_warm_concurrent_and_corrupt_acquisition() {
    let fixture = Fixture::new();
    let mut a = fixture.worker().spawn().unwrap();
    let mut b = fixture.worker().spawn().unwrap();
    assert!(a.wait().unwrap().success());
    assert!(b.wait().unwrap().success());
    assert!(fixture.object().join("manifest.json").is_file());
    assert_eq!(fs::read_dir(fixture.root.join("cache/objects")).unwrap().count(), 1);
    let warm = fixture.worker().env("SEARCHKERNEL_RUNTIME_ORIGIN", "file:///unavailable-runtime").output().unwrap();
    assert!(warm.status.success(), "{}", String::from_utf8_lossy(&warm.stderr));
    // Corruption must never be accepted; a failed repair leaves no usable object.
    fs::write(fixture.object().join("lib/libsearchkernel_vespa.so.2"), b"corrupt").unwrap();
    let corrupt = fixture.worker().env("SEARCHKERNEL_RUNTIME_ORIGIN", "file:///unavailable-runtime").output().unwrap();
    assert!(!corrupt.status.success());
    assert!(!fixture.object().exists());
    assert!(fixture.worker().output().unwrap().status.success());
}

#[test]
fn wrong_artifact_digest_and_override_fail_closed() {
    let mut fixture = Fixture::new();
    fixture.descriptor["artifact_sha256"] = json!("0".repeat(64));
    fixture.write_descriptor();
    let result = fixture.worker().output().unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("artifact-digest-mismatch") || String::from_utf8_lossy(&result.stderr).contains("artifact-digest-mismatch"));
    assert!(!fixture.object().exists());
    // An explicit directory must still match the package-pinned payload hash.
    let mut manifest = fixture.identity.clone();
    fs::write(fixture.root.join("runtime/lib/libsearchkernel_vespa.so.2"), b"different").unwrap();
    manifest["payload_sha256"] = json!(digest(&fixture.root.join("runtime")));
    fs::write(fixture.root.join("runtime/manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
    let result = fixture.worker().env("SEARCHKERNEL_RUNTIME_DIR", fixture.root.join("runtime")).output().unwrap();
    assert!(!result.status.success());
}

#[test]
fn cargo_target_and_abi_are_checked_before_acquisition() {
    let fixture = Fixture::new();
    let path = fixture.root.join("runtime.json");
    let target = fixture.identity["native_target"].as_str().unwrap();
    assert!(consumer_runtime::ensure_runtime_for_build(&path, "different-target", target, 16, "0.1.0").unwrap_err().contains("platform-mismatch"));
    assert!(consumer_runtime::ensure_runtime_for_build(&path, target, target, 999, "0.1.0").unwrap_err().contains("identity-mismatch"));
    assert!(consumer_runtime::ensure_runtime_for_build(&path, target, target, 16, "different-version").unwrap_err().contains("identity-mismatch"));
}

#[test]
fn sidecars_are_identity_bound_atomic_and_detect_corruption() {
    consumer_layout::producer_directory().unwrap();
    let fixture = Fixture::new();
    let identity = fixture.identity["runtime_identity"].as_str().unwrap();
    let destination = fixture.root.join("executable");
    let sidecar = consumer_runtime::stage_libraries(&fixture.root.join("runtime/lib"), &destination, identity).unwrap();
    assert_eq!(sidecar, destination.join(consumer_layout::runtime_directory(identity).unwrap()));
    assert_eq!(consumer_layout::adapter_soname(identity).unwrap(), format!("$ORIGIN/.searchkernel-native/{identity}/libsearchkernel_vespa.so.2"));
    let again = consumer_runtime::stage_libraries(&fixture.root.join("runtime/lib"), &destination, identity).unwrap();
    assert_eq!(sidecar, again);
    fs::write(sidecar.join("libsearchkernel_vespa.so.2"), b"wrong").unwrap();
    assert!(consumer_runtime::stage_libraries(&fixture.root.join("runtime/lib"), &destination, identity).unwrap_err().contains("digest-mismatch"));
    assert_eq!(fs::read(fixture.root.join("runtime/lib/libsearchkernel_vespa.so.2")).unwrap(), b"adapter");
    assert!(consumer_layout::runtime_directory("../other-generation").is_err());
}

#[test]
fn actual_elf_soname_is_required_and_malformed_headers_fail_safely() {
    let fixture = Fixture::new();
    let identity = fixture.identity["runtime_identity"].as_str().unwrap();
    let source = fixture.root.join("fake-adapter.c");
    let library = fixture.root.join("fake-adapter.so");
    fs::write(&source, "unsigned int sk_native_abi_version(void) { return 16; }\n").unwrap();
    let soname = consumer_layout::adapter_soname(identity).unwrap();
    let mut compiler = cc::Build::new().cargo_metadata(false)
        .target("x86_64-unknown-linux-gnu").host("x86_64-unknown-linux-gnu").opt_level(0)
        .get_compiler().to_command();
    let status = compiler.arg("-shared").arg("-fPIC").arg(&source)
        .arg(format!("-Wl,-soname,{soname}")).arg("-o").arg(&library).status().unwrap();
    assert!(status.success());
    consumer_layout::verify_adapter_soname(&library, identity).unwrap();
    let native_directory = fixture.root.join(consumer_layout::runtime_directory(identity).unwrap());
    fs::create_dir_all(&native_directory).unwrap();
    fs::copy(&library, native_directory.join("libsearchkernel_vespa.so.2")).unwrap();
    // Link and execute the exact packaged probe against a deliberately wrong
    // ABI implementation. Matching metadata/SONAME alone must not suffice.
    let probe_source = fixture.root.join("probe.c");
    let probe = fixture.root.join("probe");
    for (abi, expected_status) in [(16, 0), (17, 1)] {
        fs::write(&probe_source, consumer_layout::abi_probe_source(abi)).unwrap();
        let mut compiler = cc::Build::new().cargo_metadata(false)
            .target("x86_64-unknown-linux-gnu").host("x86_64-unknown-linux-gnu").opt_level(0)
            .get_compiler().to_command();
        assert!(compiler.arg(&probe_source).arg(&library).arg("-o").arg(&probe).status().unwrap().success());
        assert_eq!(Command::new(&probe).env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD").status().unwrap().code(), Some(expected_status));
    }
    let different = format!("skrt-v1-{}", "f".repeat(64));
    assert!(consumer_layout::verify_adapter_soname(&library, &different).unwrap_err().contains("identity-mismatch"));
    let data = fs::read(library).unwrap();
    for length in [0, 6, 30, 58] { assert!(consumer_layout::elf_soname(&data[..length]).is_err()); }
    let mut malformed = data;
    malformed[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(consumer_layout::elf_soname(&malformed).is_err());
}

#[test]
fn unsafe_archive_members_never_publish_a_runtime() {
    for variant in ["escape", "duplicate", "link-parent", "linked-child"] {
        let mut fixture = Fixture::new();
        let archive_path = url::Url::parse(fixture.descriptor["artifact_url"].as_str().unwrap()).unwrap().to_file_path().unwrap();
        let mut archive = tar::Builder::new(GzEncoder::new(fs::File::create(&archive_path).unwrap(), Compression::default()));
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o644);
        match variant {
            "escape" => {
                header.set_size(1);
                header.set_path("safe").unwrap();
                header.as_mut_bytes()[..9].copy_from_slice(b"../escape");
                header.set_cksum();
                archive.append(&header, &b"x"[..]).unwrap();
            }
            "duplicate" => {
                header.set_size(1);
                header.set_cksum();
                archive.append_data(&mut header, "lib/duplicate", &b"x"[..]).unwrap();
                archive.append_data(&mut header, "lib/duplicate", &b"y"[..]).unwrap();
            }
            "linked-child" => {
                header.set_size(0);
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_link_name("safe").unwrap();
                header.set_cksum();
                archive.append_data(&mut header, "lib/link", std::io::empty()).unwrap();
                header.set_entry_type(tar::EntryType::Regular);
                header.set_size(1);
                header.set_cksum();
                archive.append_data(&mut header, "lib/link/child", &b"x"[..]).unwrap();
            }
            _ => {
                header.set_size(0);
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_link_name("../../escape").unwrap();
                header.set_cksum();
                archive.append_data(&mut header, "lib/escape", std::io::empty()).unwrap();
            }
        }
        archive.into_inner().unwrap().finish().unwrap();
        fixture.descriptor["artifact_sha256"] = json!(format!("{:x}", Sha256::digest(fs::read(archive_path).unwrap())));
        fixture.write_descriptor();
        let result = fixture.worker().output().unwrap();
        assert!(!result.status.success());
        let diagnostic = format!("{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
        assert!(diagnostic.contains("artifact-invalid"), "{diagnostic}");
        assert!(!fixture.object().exists());
    }
}
