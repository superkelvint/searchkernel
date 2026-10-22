use flate2::read::GzDecoder;
use fs2::FileExt;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
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
    let mut material = BTreeMap::new();
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
        || artifact
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            != Some(expected.as_str())
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
        } else if !metadata.is_file() {
            return Err(resolution_error(
                "runtime-invalid",
                "runtime contains a special filesystem entry",
            ));
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
    runtime_files(&base, root, &mut files)?;
    files.sort();
    let mut hash = Sha256::new();
    for path in files {
        let relative = path
            .strip_prefix(root)
            .map_err(|error| resolution_error("runtime-invalid", error))?
            .to_str()
            .ok_or_else(|| resolution_error("runtime-invalid", "runtime paths must be UTF-8"))?
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
    if let Some(pinned) = descriptor.get("payload_sha256").and_then(Value::as_str) {
        if pinned != expected {
            return Err(resolution_error(
                "digest-mismatch",
                "manifest differs from package-pinned payload digest",
            ));
        }
    }
    if payload_digest(root)? != expected {
        return Err(resolution_error(
            "digest-mismatch",
            "runtime payload digest differs",
        ));
    }
    Ok(root_real)
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
        let path: PathBuf = path
            .components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(name),
                _ => None,
            })
            .collect();
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
        if !kind.is_file() && !kind.is_dir() && !kind.is_symlink() {
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
        if members.iter().any(|(previous, _, _)| previous == &path) {
            return Err(resolution_error(
                "artifact-invalid",
                "duplicate archive member",
            ));
        }
        members.push((path, kind, link));
    }
    let links: Vec<PathBuf> = members
        .iter()
        .filter(|(_, kind, _)| kind.is_symlink())
        .map(|(path, _, _)| path.clone())
        .collect();
    for (path, _, _) in &members {
        if links
            .iter()
            .any(|link| path != link && path.starts_with(link))
        {
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
    if let Some(path) = env::var_os("SEARCHKERNEL_RUNTIME_CACHE").filter(|path| !path.is_empty()) {
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
    if let Some(origin) = env::var_os("SEARCHKERNEL_RUNTIME_ORIGIN").filter(|path| !path.is_empty())
    {
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

pub fn load_descriptor(descriptor_path: &Path) -> Result<Value, String> {
    let descriptor: Value = serde_json::from_slice(
        &fs::read(descriptor_path)
            .map_err(|error| resolution_error("descriptor-invalid", error))?,
    )
    .map_err(|error| resolution_error("descriptor-invalid", error))?;
    validate_descriptor(&descriptor)?;
    Ok(descriptor)
}

pub fn ensure_runtime_for_build(
    descriptor_path: &Path,
    target: &str,
    host: &str,
    abi: u32,
    version: &str,
) -> Result<(PathBuf, String), String> {
    let descriptor = load_descriptor(descriptor_path)?;
    if target != host || string(&descriptor, "native_target")? != target {
        return Err(resolution_error(
            "platform-mismatch",
            "cross-compilation or unqualified Cargo target",
        ));
    }
    if descriptor.get("native_abi_version").and_then(Value::as_u64) != Some(u64::from(abi))
        || string(&descriptor, "searchkernel_version")? != version
    {
        return Err(resolution_error(
            "identity-mismatch",
            "runtime descriptor does not match this Cargo package ABI/version",
        ));
    }
    let digest = string(&descriptor, "payload_sha256")?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(resolution_error(
            "descriptor-invalid",
            "missing or invalid package-pinned payload digest",
        ));
    }
    let runtime = ensure_runtime(&descriptor)?;
    Ok((runtime, string(&descriptor, "runtime_identity")?.to_owned()))
}

pub fn ensure_runtime(descriptor: &Value) -> Result<PathBuf, String> {
    validate_descriptor(descriptor)?;
    if let Some(runtime) = env::var_os("SEARCHKERNEL_RUNTIME_DIR").filter(|path| !path.is_empty()) {
        return validate_runtime(Path::new(&runtime), descriptor);
    }
    let identity = string(descriptor, "runtime_identity")?;
    let cache = cache_root();
    let final_path = cache.join("objects").join(identity);
    if let Ok(path) = validate_runtime(&final_path, descriptor) {
        return Ok(path);
    }
    fs::create_dir_all(cache.join("locks"))
        .map_err(|error| resolution_error("cache-io-failed", error))?;
    let lock_path = cache.join("locks").join(format!("{identity}.lock"));
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| resolution_error("cache-io-failed", error))?;
    lock.lock_exclusive()
        .map_err(|error| resolution_error("cache-io-failed", error))?;
    if let Ok(path) = validate_runtime(&final_path, descriptor) {
        return Ok(path);
    }
    remove_entry(&final_path)?;
    let staging_root = cache.join("staging");
    fs::create_dir_all(&staging_root)
        .map_err(|error| resolution_error("cache-io-failed", error))?;
    // The identity lock excludes other active installers for this object.
    // Recover temporary entries left behind by a terminated acquisition.
    for entry in
        fs::read_dir(&staging_root).map_err(|error| resolution_error("cache-io-failed", error))?
    {
        let entry = entry.map_err(|error| resolution_error("cache-io-failed", error))?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(&format!("{identity}.")))
        {
            remove_entry(&entry.path())?;
        }
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let stage = staging_root.join(format!("{identity}.{}.{nonce}", std::process::id()));
    fs::create_dir(&stage).map_err(|error| resolution_error("cache-io-failed", error))?;
    let result = (|| {
        let archive_path = stage.join("runtime.tar.gz");
        download(&resolved_artifact_url(descriptor)?, &archive_path)?;
        if sha256_file(&archive_path)? != string(descriptor, "artifact_sha256")? {
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
        validate_runtime(&unpack, descriptor)?;
        fs::create_dir_all(final_path.parent().unwrap())
            .map_err(|error| resolution_error("cache-io-failed", error))?;
        fs::rename(&unpack, &final_path)
            .map_err(|error| resolution_error("cache-io-failed", error))?;
        validate_runtime(&final_path, descriptor)
    })();
    let _ = remove_entry(&stage);
    result
}

fn copy_libraries(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir(destination).map_err(|error| resolution_error("cache-io-failed", error))?;
    for item in fs::read_dir(source).map_err(|error| resolution_error("runtime-invalid", error))? {
        let item = item.map_err(|error| resolution_error("runtime-invalid", error))?;
        let path = item.path();
        let target = destination.join(item.file_name());
        let metadata =
            fs::metadata(&path).map_err(|error| resolution_error("runtime-invalid", error))?;
        if metadata.is_file() {
            // Copy rather than hard-link: editing a deployed sidecar must never
            // mutate the shared, validated runtime cache object.
            fs::copy(path, target).map_err(|error| resolution_error("cache-io-failed", error))?;
        } else if metadata.is_dir()
            && !item
                .file_type()
                .map_err(|error| error.to_string())?
                .is_symlink()
        {
            copy_libraries(&path, &target)?;
        } else {
            return Err(resolution_error(
                "runtime-invalid",
                "unsupported library entry",
            ));
        }
    }
    Ok(())
}

pub fn stage_libraries(
    source: &Path,
    executable_dir: &Path,
    identity: &str,
) -> Result<PathBuf, String> {
    let relative = super::consumer_layout::runtime_directory(identity)?;
    let final_path = executable_dir.join(relative);
    let parent = final_path
        .parent()
        .ok_or("missing runtime sidecar parent")?;
    fs::create_dir_all(parent).map_err(|error| resolution_error("cache-io-failed", error))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(parent.join(format!("{identity}.lock")))
        .map_err(|error| resolution_error("cache-io-failed", error))?;
    lock.lock_exclusive()
        .map_err(|error| resolution_error("cache-io-failed", error))?;
    let expected = payload_digest(source)?;
    if fs::symlink_metadata(&final_path).is_ok() {
        if fs::symlink_metadata(&final_path)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
            || payload_digest(&final_path)? != expected
        {
            return Err(resolution_error(
                "digest-mismatch",
                "existing immutable Cargo sidecar differs; use a clean Cargo target directory",
            ));
        }
        return Ok(final_path);
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let stage = parent.join(format!(".{identity}.{}.{nonce}", std::process::id()));
    let result = (|| {
        copy_libraries(source, &stage)?;
        if payload_digest(&stage)? != expected {
            return Err(resolution_error(
                "digest-mismatch",
                "staged Cargo sidecar differs",
            ));
        }
        fs::rename(&stage, &final_path)
            .map_err(|error| resolution_error("cache-io-failed", error))?;
        Ok(final_path)
    })();
    let _ = remove_entry(&stage);
    result
}
