use flate2::read::GzDecoder;
use fs2::FileExt;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tar::{Archive, EntryType};
use url::Url;

const FORMAT_VERSION: u64 = 1;
const IDENTITY_MATERIAL_KEYS: &[&str] = &[
    "searchkernel_version",
    "searchkernel_revision",
    "runtime_package_generation",
    "platform",
    "native_target",
    "build_environment",
    "native_abi_version",
    "vespa_commit",
];
const IDENTITY_KEYS: &[&str] = &[
    "searchkernel_version",
    "searchkernel_revision",
    "runtime_package_generation",
    "platform",
    "native_target",
    "build_environment",
    "native_abi_version",
    "vespa_commit",
    "runtime_identity",
];

fn resolution_error(code: &str, message: impl std::fmt::Display) -> String {
    format!("{code}: {message}")
}

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            resolution_error(
                "descriptor-invalid",
                format!("{key} must be a non-empty string"),
            )
        })
}

fn canonical_identity_material(value: &Value) -> Result<Vec<u8>, String> {
    let mut material = Map::new();
    for key in IDENTITY_MATERIAL_KEYS {
        let field = value.get(*key).ok_or_else(|| {
            resolution_error("descriptor-invalid", format!("missing field {key}"))
        })?;
        material.insert((*key).to_owned(), field.clone());
    }
    serde_json::to_vec(&material).map_err(|error| resolution_error("descriptor-invalid", error))
}

fn runtime_identity(value: &Value) -> Result<String, String> {
    let bytes = canonical_identity_material(value)?;
    Ok(format!("skrt-v1-{:x}", Sha256::digest(bytes)))
}

fn validate_descriptor(value: &Value) -> Result<(), String> {
    if !value.is_object()
        || value.get("format_version").and_then(Value::as_u64) != Some(FORMAT_VERSION)
    {
        return Err(resolution_error(
            "descriptor-invalid",
            "unsupported or malformed descriptor format",
        ));
    }
    for key in [
        "searchkernel_version",
        "searchkernel_revision",
        "platform",
        "native_target",
        "build_environment",
        "vespa_commit",
    ] {
        string(value, key)?;
    }
    for key in ["runtime_package_generation", "native_abi_version"] {
        if value
            .get(key)
            .and_then(Value::as_u64)
            .filter(|number| *number > 0)
            .is_none()
        {
            return Err(resolution_error(
                "descriptor-invalid",
                format!("{key} must be a positive integer"),
            ));
        }
    }
    let identity = string(value, "runtime_identity")?;
    if identity.len() != 72
        || !identity.starts_with("skrt-v1-")
        || !identity[8..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(resolution_error(
            "descriptor-invalid",
            "runtime_identity has invalid syntax",
        ));
    }
    if runtime_identity(value)? != identity {
        return Err(resolution_error(
            "descriptor-invalid",
            "runtime_identity does not match identity material",
        ));
    }
    let digest = string(value, "artifact_sha256")?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(resolution_error(
            "descriptor-invalid",
            "artifact_sha256 must be lowercase SHA-256 hex",
        ));
    }
    if !cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_env = "gnu"
    )) || string(value, "platform")? != "linux-x86_64-glibc"
        || string(value, "native_target")? != "x86_64-unknown-linux-gnu"
    {
        return Err(resolution_error(
            "platform-mismatch",
            "descriptor does not match a qualified glibc host",
        ));
    }
    let expected = expected_artifact_filename(value)?;
    let artifact = Url::parse(string(value, "artifact_url")?)
        .map_err(|error| resolution_error("origin-invalid", error))?;
    if !matches!(artifact.scheme(), "https" | "file")
        || (artifact.scheme() == "https" && artifact.host_str().is_none())
        || artifact.path_segments().and_then(Iterator::last) != Some(expected.as_str())
    {
        return Err(resolution_error(
            "origin-invalid",
            "artifact URL must name the immutable runtime artifact",
        ));
    }
    Ok(())
}

fn expected_artifact_filename(value: &Value) -> Result<String, String> {
    Ok(format!(
        "searchkernel-runtime-{}-{}.tar.gz",
        string(value, "runtime_identity")?,
        string(value, "platform")?
    ))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| resolution_error("cache-io-failed", error))?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 1024 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| resolution_error("cache-io-failed", error))?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn runtime_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries = fs::read_dir(current)
        .map_err(|error| resolution_error("runtime-invalid", error))?
        .map(|entry| entry.map_err(|error| resolution_error("runtime-invalid", error)))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| resolution_error("runtime-invalid", error))?;
        if metadata.file_type().is_symlink() {
            let target = fs::canonicalize(&path).map_err(|error| {
                resolution_error(
                    "runtime-invalid",
                    format!("runtime symlink cannot be resolved: {error}"),
                )
            })?;
            ensure_within(
                root,
                &target,
                "runtime-invalid",
                "runtime symlink escapes runtime root",
            )?;
            let target_metadata = fs::metadata(&target)
                .map_err(|error| resolution_error("runtime-invalid", error))?;
            if target_metadata.is_file()
                && path.file_name().and_then(|name| name.to_str()) != Some("manifest.json")
            {
                files.push(path);
            }
        } else if metadata.is_dir() {
            runtime_files(root, &path, files)?;
        } else if metadata.is_file()
            && path.file_name().and_then(|name| name.to_str()) != Some("manifest.json")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn ensure_within(root: &Path, path: &Path, code: &str, message: &str) -> Result<(), String> {
    if path == root || path.starts_with(root) {
        Ok(())
    } else {
        Err(resolution_error(code, message))
    }
}

fn payload_digest(root: &Path) -> Result<String, String> {
    let base =
        fs::canonicalize(root).map_err(|error| resolution_error("runtime-invalid", error))?;
    let mut files = Vec::new();
    runtime_files(root, root, &mut files)?;
    files.sort();
    let mut hash = Sha256::new();
    for path in files {
        let relative = path
            .strip_prefix(root)
            .map_err(|error| resolution_error("runtime-invalid", error))?
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        let actual =
            fs::canonicalize(&path).map_err(|error| resolution_error("runtime-invalid", error))?;
        ensure_within(
            &base,
            &actual,
            "runtime-invalid",
            "runtime symlink escapes runtime root",
        )?;
        let data = fs::read(actual).map_err(|error| resolution_error("runtime-invalid", error))?;
        hash.update((relative.len() as u64).to_be_bytes());
        hash.update(relative.as_bytes());
        hash.update((data.len() as u64).to_be_bytes());
        hash.update(data);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn validate_runtime(root: &Path, descriptor: &Value) -> Result<PathBuf, String> {
    let manifest_path = root.join("manifest.json");
    let manifest_metadata = fs::symlink_metadata(&manifest_path).map_err(|error| {
        resolution_error("runtime-invalid", format!("manifest unavailable: {error}"))
    })?;
    let manifest_target = if manifest_metadata.file_type().is_symlink() {
        fs::canonicalize(&manifest_path).map_err(|error| {
            resolution_error(
                "runtime-invalid",
                format!("manifest symlink cannot be resolved: {error}"),
            )
        })?
    } else {
        fs::canonicalize(&manifest_path)
            .map_err(|error| resolution_error("runtime-invalid", error))?
    };
    let root_real =
        fs::canonicalize(root).map_err(|error| resolution_error("runtime-invalid", error))?;
    ensure_within(
        &root_real,
        &manifest_target,
        "runtime-invalid",
        "runtime manifest symlink escapes runtime root",
    )?;
    if !fs::metadata(&manifest_target)
        .map_err(|error| resolution_error("runtime-invalid", error))?
        .is_file()
    {
        return Err(resolution_error(
            "runtime-invalid",
            "manifest must be a regular file",
        ));
    }
    let manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| {
        resolution_error("runtime-invalid", format!("manifest unavailable: {error}"))
    })?)
    .map_err(|error| {
        resolution_error("runtime-invalid", format!("manifest unavailable: {error}"))
    })?;
    if !manifest.is_object()
        || manifest.get("format_version").and_then(Value::as_u64) != Some(FORMAT_VERSION)
    {
        return Err(resolution_error(
            "runtime-invalid",
            "unsupported runtime manifest format",
        ));
    }
    for key in IDENTITY_KEYS {
        if manifest.get(*key) != descriptor.get(*key) {
            return Err(resolution_error(
                "identity-mismatch",
                format!("{key} differs"),
            ));
        }
    }
    let base =
        fs::canonicalize(root).map_err(|error| resolution_error("runtime-invalid", error))?;
    let required = [
        "bin/searchkerneld",
        "bin/searchkerneld.real",
        "lib/libsearchkernel_vespa.so.2",
    ];
    for relative in required {
        let path = root.join(relative);
        let metadata = fs::metadata(&path).map_err(|_| {
            resolution_error(
                "runtime-invalid",
                format!("required runtime member missing: {relative}"),
            )
        })?;
        if !metadata.is_file() {
            return Err(resolution_error(
                "runtime-invalid",
                format!("required runtime member missing: {relative}"),
            ));
        }
        let target =
            fs::canonicalize(&path).map_err(|error| resolution_error("runtime-invalid", error))?;
        ensure_within(
            &base,
            &target,
            "runtime-invalid",
            "required runtime member escapes runtime root",
        )?;
    }
    for relative in ["bin/searchkerneld", "bin/searchkerneld.real"] {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(root.join(relative))
                .map_err(|error| resolution_error("runtime-invalid", error))?
                .permissions()
                .mode()
                & 0o111
                == 0
            {
                return Err(resolution_error(
                    "runtime-invalid",
                    format!("required runtime executable is not executable: {relative}"),
                ));
            }
        }
    }
    let expected = manifest
        .get("payload_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| resolution_error("runtime-invalid", "manifest payload_sha256 is missing"))?;
    if payload_digest(root)? != expected {
        return Err(resolution_error(
            "digest-mismatch",
            "runtime payload digest differs",
        ));
    }
    Ok(root.to_path_buf())
}

fn validate_archive(archive_path: &Path) -> Result<(), String> {
    let file =
        File::open(archive_path).map_err(|error| resolution_error("artifact-invalid", error))?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    let mut members: Vec<(PathBuf, EntryType, Option<PathBuf>)> = Vec::new();
    for item in archive
        .entries()
        .map_err(|error| resolution_error("artifact-invalid", error))?
    {
        let entry = item.map_err(|error| resolution_error("artifact-invalid", error))?;
        let path = entry
            .path()
            .map_err(|error| resolution_error("artifact-invalid", error))?
            .into_owned();
        if !safe_relative_path(&path) {
            return Err(resolution_error(
                "artifact-invalid",
                "archive path escapes runtime root",
            ));
        }
        let kind = entry.header().entry_type();
        if kind.is_hard_link() {
            return Err(resolution_error(
                "artifact-invalid",
                "archive hard links are not permitted",
            ));
        }
        if kind.is_block_special() || kind.is_character_special() || kind.is_fifo() {
            return Err(resolution_error(
                "artifact-invalid",
                "archive special files are not permitted",
            ));
        }
        if !kind.is_file() && !kind.is_dir() && !kind.is_symlink() && !kind.is_gnu_sparse() {
            return Err(resolution_error(
                "artifact-invalid",
                "unsupported archive member type",
            ));
        }
        let link = if kind.is_symlink() {
            let name = entry
                .link_name()
                .map_err(|error| resolution_error("artifact-invalid", error))?
                .ok_or_else(|| {
                    resolution_error("artifact-invalid", "archive symlink has no target")
                })?;
            let target = normalized_link_target(&path, &name).ok_or_else(|| {
                resolution_error("artifact-invalid", "archive link escapes runtime root")
            })?;
            Some(target)
        } else {
            None
        };
        members.push((path, kind, link));
    }
    let links: Vec<PathBuf> = members
        .iter()
        .filter(|(_, kind, _)| kind.is_symlink())
        .map(|(path, _, _)| path.clone())
        .collect();
    for (path, kind, _) in &members {
        if !kind.is_symlink() && links.iter().any(|link| path.starts_with(link)) {
            return Err(resolution_error(
                "artifact-invalid",
                "archive member traverses an archive link",
            ));
        }
    }
    Ok(())
}

fn safe_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn normalized_link_target(link_path: &Path, target: &Path) -> Option<PathBuf> {
    if target.is_absolute() {
        return None;
    }
    let mut components: Vec<std::ffi::OsString> = link_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_owned()),
            _ => None,
        })
        .collect();
    for component in target.components() {
        match component {
            Component::Normal(value) => components.push(value.to_owned()),
            Component::CurDir => {}
            Component::ParentDir => {
                components.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(components.into_iter().collect())
}

fn cache_root() -> PathBuf {
    if let Some(path) = env::var_os("SEARCHKERNEL_RUNTIME_CACHE") {
        return PathBuf::from(path);
    }
    let base = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(|| PathBuf::from(".cache"));
    base.join("searchkernel/runtime")
}

fn resolved_artifact_url(descriptor: &Value) -> Result<Url, String> {
    let expected_name = expected_artifact_filename(descriptor)?;
    if let Some(origin) = env::var_os("SEARCHKERNEL_RUNTIME_ORIGIN") {
        let origin = origin.to_string_lossy();
        let mut base =
            Url::parse(&origin).map_err(|error| resolution_error("origin-invalid", error))?;
        if !matches!(base.scheme(), "https" | "file")
            || (base.scheme() == "https" && base.host_str().is_none())
        {
            return Err(resolution_error(
                "origin-invalid",
                "runtime origin scheme is not permitted",
            ));
        }
        if !base.path().ends_with('/') {
            base.set_path(&format!("{}/", base.path()));
        }
        return base
            .join(&expected_name)
            .map_err(|error| resolution_error("origin-invalid", error));
    }
    Url::parse(string(descriptor, "artifact_url")?)
        .map_err(|error| resolution_error("origin-invalid", error))
}

fn download(url: &Url, destination: &Path) -> Result<(), String> {
    if url.scheme() == "file" {
        let source = url
            .to_file_path()
            .map_err(|_| resolution_error("origin-invalid", "file runtime origin is invalid"))?;
        fs::copy(source, destination).map_err(|error| resolution_error("acquire-failed", error))?;
        return Ok(());
    }
    let response = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .build()
        .get(url.as_str())
        .call()
        .map_err(|error| resolution_error("acquire-failed", error))?;
    let mut reader = response.into_reader();
    let mut file =
        File::create(destination).map_err(|error| resolution_error("cache-io-failed", error))?;
    std::io::copy(&mut reader, &mut file)
        .map_err(|error| resolution_error("acquire-failed", error))?;
    file.flush()
        .map_err(|error| resolution_error("cache-io-failed", error))?;
    Ok(())
}

fn remove_entry(path: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(resolution_error("cache-io-failed", error)),
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path).map_err(|error| resolution_error("cache-io-failed", error))
    } else {
        fs::remove_file(path).map_err(|error| resolution_error("cache-io-failed", error))
    }
}

pub fn ensure_runtime(descriptor_path: &Path) -> Result<PathBuf, String> {
    let descriptor: Value = serde_json::from_slice(
        &fs::read(descriptor_path)
            .map_err(|error| resolution_error("descriptor-invalid", error))?,
    )
    .map_err(|error| resolution_error("descriptor-invalid", error))?;
    validate_descriptor(&descriptor)?;
    if let Some(runtime) = env::var_os("SEARCHKERNEL_RUNTIME_DIR") {
        return validate_runtime(Path::new(&runtime), &descriptor);
    }
    let identity = string(&descriptor, "runtime_identity")?;
    let cache = cache_root();
    let final_path = cache.join("objects").join(identity);
    if validate_runtime(&final_path, &descriptor).is_ok() {
        return Ok(final_path);
    }
    fs::create_dir_all(cache.join("locks"))
        .map_err(|error| resolution_error("cache-io-failed", error))?;
    let lock_path = cache.join("locks").join(format!("{identity}.lock"));
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| resolution_error("cache-io-failed", error))?;
    lock.lock_exclusive()
        .map_err(|error| resolution_error("cache-io-failed", error))?;
    if validate_runtime(&final_path, &descriptor).is_ok() {
        return Ok(final_path);
    }
    remove_entry(&final_path)?;
    let staging_root = cache.join("staging");
    fs::create_dir_all(&staging_root)
        .map_err(|error| resolution_error("cache-io-failed", error))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let stage = staging_root.join(format!("{identity}.{}.{nonce}", std::process::id()));
    fs::create_dir(&stage).map_err(|error| resolution_error("cache-io-failed", error))?;
    let result = (|| {
        let archive_path = stage.join("runtime.tar.gz");
        download(&resolved_artifact_url(&descriptor)?, &archive_path)?;
        if sha256_file(&archive_path)? != string(&descriptor, "artifact_sha256")? {
            return Err(resolution_error(
                "artifact-digest-mismatch",
                "downloaded archive digest differs",
            ));
        }
        validate_archive(&archive_path)?;
        let unpack = stage.join("unpack");
        fs::create_dir(&unpack).map_err(|error| resolution_error("cache-io-failed", error))?;
        let file = File::open(&archive_path)
            .map_err(|error| resolution_error("artifact-invalid", error))?;
        let mut archive = Archive::new(GzDecoder::new(file));
        archive
            .unpack(&unpack)
            .map_err(|error| resolution_error("artifact-invalid", error))?;
        validate_runtime(&unpack, &descriptor)?;
        fs::create_dir_all(final_path.parent().unwrap())
            .map_err(|error| resolution_error("cache-io-failed", error))?;
        fs::rename(&unpack, &final_path)
            .map_err(|error| resolution_error("cache-io-failed", error))?;
        validate_runtime(&final_path, &descriptor)
    })();
    let _ = remove_entry(&stage);
    result
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_runtime, expected_artifact_filename, payload_digest, runtime_identity, sha256_file,
        validate_archive, validate_descriptor, validate_runtime, Url,
    };
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use serde_json::{json, Value};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tar::Builder;

    const CONFORMANCE: &str = include_str!("../../../../build/consumer-runtime/conformance.json");

    fn temp_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "searchkernel-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn fixture_identity() -> (Value, Value) {
        let conformance: Value = serde_json::from_str(CONFORMANCE).unwrap();
        let vector = &conformance["vectors"][0];
        let material = vector["identity_material"].clone();
        let identity = json!({
            "format_version": 1,
            "searchkernel_version": material["searchkernel_version"],
            "searchkernel_revision": material["searchkernel_revision"],
            "runtime_package_generation": material["runtime_package_generation"],
            "platform": material["platform"],
            "native_target": material["native_target"],
            "build_environment": material["build_environment"],
            "native_abi_version": material["native_abi_version"],
            "vespa_commit": material["vespa_commit"],
            "runtime_identity": vector["runtime_identity"],
        });
        (conformance, identity)
    }

    fn write_valid_runtime(root: &Path, identity: &Value) {
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("lib")).unwrap();
        fs::write(root.join("bin/searchkerneld"), b"launcher").unwrap();
        fs::write(root.join("bin/searchkerneld.real"), b"daemon").unwrap();
        fs::write(root.join("lib/libsearchkernel_vespa.so.2"), b"adapter").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                root.join("bin/searchkerneld"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
            fs::set_permissions(
                root.join("bin/searchkerneld.real"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        let mut manifest = identity.as_object().unwrap().clone();
        manifest.insert(
            "payload_sha256".to_owned(),
            json!(payload_digest(root).unwrap()),
        );
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
    }

    fn descriptor(identity: &Value) -> Value {
        let name = format!(
            "searchkernel-runtime-{}-{}.tar.gz",
            identity["runtime_identity"].as_str().unwrap(),
            identity["platform"].as_str().unwrap()
        );
        json!({
            "format_version": 1,
            "searchkernel_version": identity["searchkernel_version"],
            "searchkernel_revision": identity["searchkernel_revision"],
            "runtime_package_generation": identity["runtime_package_generation"],
            "platform": identity["platform"],
            "native_target": identity["native_target"],
            "build_environment": identity["build_environment"],
            "native_abi_version": identity["native_abi_version"],
            "vespa_commit": identity["vespa_commit"],
            "runtime_identity": identity["runtime_identity"],
            "artifact_url": format!("https://example.invalid/{name}"),
            "artifact_sha256": "a".repeat(64)
        })
    }

    #[test]
    fn identity_cache_and_runtime_validation_use_shared_conformance_fixtures() {
        let (conformance, identity) = fixture_identity();
        assert_eq!(
            runtime_identity(&identity).unwrap(),
            conformance["vectors"][0]["runtime_identity"]
                .as_str()
                .unwrap()
        );
        let vector = &conformance["vectors"][0];
        let runtime_id = vector["runtime_identity"].as_str().unwrap();
        assert_eq!(
            vector["cache_relative_path"],
            format!("objects/{runtime_id}")
        );
        assert_eq!(
            vector["lock_relative_path"],
            format!("locks/{runtime_id}.lock")
        );

        let cases = conformance["runtime_validation_cases"].as_array().unwrap();
        for fixture in cases {
            let root = temp_root(fixture["name"].as_str().unwrap());
            write_valid_runtime(&root, &identity);
            let mut manifest: Value =
                serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
            match fixture["mutation"].as_str().unwrap() {
                "manifest-format" => manifest["format_version"] = json!(99),
                "non-executable" => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        fs::set_permissions(
                            root.join("bin/searchkerneld.real"),
                            fs::Permissions::from_mode(0o644),
                        )
                        .unwrap();
                    }
                }
                "escaping-symlink" => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::symlink;
                        fs::remove_file(root.join("lib/libsearchkernel_vespa.so.2")).unwrap();
                        let outside = root
                            .parent()
                            .unwrap()
                            .join("searchkernel-outside-runtime.so");
                        fs::write(&outside, b"outside").unwrap();
                        symlink(&outside, root.join("lib/libsearchkernel_vespa.so.2")).unwrap();
                    }
                }
                "identity-mismatch" => manifest["searchkernel_revision"] = json!("different"),
                "payload-digest-mismatch" => manifest["payload_sha256"] = json!("0".repeat(64)),
                other => panic!("unknown shared runtime fixture mutation {other}"),
            }
            if !matches!(
                fixture["mutation"].as_str().unwrap(),
                "non-executable" | "escaping-symlink"
            ) {
                fs::write(
                    root.join("manifest.json"),
                    serde_json::to_vec(&manifest).unwrap(),
                )
                .unwrap();
            }
            let error = validate_runtime(&root, &descriptor(&identity)).unwrap_err();
            assert!(
                error.starts_with(fixture["expected_error"].as_str().unwrap()),
                "{} expected {} but got {error}",
                fixture["name"],
                fixture["expected_error"]
            );
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn descriptor_validation_accepts_the_supported_runtime_vector() {
        let (_conformance, identity) = fixture_identity();
        validate_descriptor(&descriptor(&identity)).unwrap();
    }

    #[test]
    fn cold_cache_install_uses_safe_rust_acquisition_path() {
        let (conformance, identity) = fixture_identity();
        let root = temp_root("runtime-acquire");
        let source = root.join("source");
        write_valid_runtime(&source, &identity);
        let archive_path = root.join(expected_artifact_filename(&descriptor(&identity)).unwrap());
        let encoder = GzEncoder::new(
            fs::File::create(&archive_path).unwrap(),
            Compression::default(),
        );
        let mut archive = Builder::new(encoder);
        archive.append_dir_all("bin", source.join("bin")).unwrap();
        archive.append_dir_all("lib", source.join("lib")).unwrap();
        archive
            .append_path_with_name(source.join("manifest.json"), "manifest.json")
            .unwrap();
        archive.into_inner().unwrap().finish().unwrap();

        let mut descriptor = descriptor(&identity);
        descriptor["artifact_url"] = json!(Url::from_file_path(&archive_path).unwrap().to_string());
        descriptor["artifact_sha256"] = json!(sha256_file(&archive_path).unwrap());
        let descriptor_path = root.join("runtime.json");
        fs::write(&descriptor_path, serde_json::to_vec(&descriptor).unwrap()).unwrap();
        let cache = root.join("cache");
        let previous_runtime = std::env::var_os("SEARCHKERNEL_RUNTIME_DIR");
        let previous_cache = std::env::var_os("SEARCHKERNEL_RUNTIME_CACHE");
        let previous_origin = std::env::var_os("SEARCHKERNEL_RUNTIME_ORIGIN");
        std::env::remove_var("SEARCHKERNEL_RUNTIME_DIR");
        std::env::remove_var("SEARCHKERNEL_RUNTIME_ORIGIN");
        std::env::set_var("SEARCHKERNEL_RUNTIME_CACHE", &cache);
        let result = ensure_runtime(&descriptor_path);
        match previous_runtime {
            Some(value) => std::env::set_var("SEARCHKERNEL_RUNTIME_DIR", value),
            None => std::env::remove_var("SEARCHKERNEL_RUNTIME_DIR"),
        }
        match previous_cache {
            Some(value) => std::env::set_var("SEARCHKERNEL_RUNTIME_CACHE", value),
            None => std::env::remove_var("SEARCHKERNEL_RUNTIME_CACHE"),
        }
        match previous_origin {
            Some(value) => std::env::set_var("SEARCHKERNEL_RUNTIME_ORIGIN", value),
            None => std::env::remove_var("SEARCHKERNEL_RUNTIME_ORIGIN"),
        }

        let runtime = result.unwrap();
        assert_eq!(
            runtime,
            cache.join("objects").join(
                conformance["vectors"][0]["runtime_identity"]
                    .as_str()
                    .unwrap()
            )
        );
        validate_runtime(&runtime, &descriptor).unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_path_and_link_escapes_fail_closed() {
        use std::io;
        use tar::EntryType;

        for (label, name, link) in [
            ("path", "../escape", None),
            ("link", "lib/escape.so", Some("../../escape")),
        ] {
            let root = temp_root(&format!("runtime-archive-{label}"));
            fs::create_dir_all(&root).unwrap();
            let path = root.join("runtime.tar.gz");
            let encoder = GzEncoder::new(fs::File::create(&path).unwrap(), Compression::default());
            let mut archive = Builder::new(encoder);
            let mut header = tar::Header::new_gnu();
            header.set_mode(0o644);
            if let Some(target) = link {
                header.set_entry_type(EntryType::Symlink);
                header.set_size(0);
                header.set_link_name(target).unwrap();
                header.set_cksum();
                archive.append_data(&mut header, name, io::empty()).unwrap();
            } else {
                header.set_entry_type(EntryType::Regular);
                header.set_size(1);
                header.set_path("safe").unwrap();
                header.as_mut_bytes()[..9].copy_from_slice(b"../escape");
                header.set_cksum();
                archive.append(&header, &b"x"[..]).unwrap();
            }
            archive.into_inner().unwrap().finish().unwrap();
            let error = validate_archive(&path).unwrap_err();
            assert!(
                error.starts_with("artifact-invalid:"),
                "unexpected archive error: {error}"
            );
            let _ = fs::remove_dir_all(root);
        }
    }
}
