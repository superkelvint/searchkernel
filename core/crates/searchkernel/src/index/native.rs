use super::{definition_from_ir, definition_to_ir, Index, IndexInner};
use crate::error::{Error, Result};
use crate::schema::IndexDefinition;
use searchkernel_ir::{Backend, BackendSession, IndexContext};
use std::sync::Arc;

#[cfg(feature = "native")]
pub(super) fn create_staging_directory(root: &std::path::Path) -> Result<std::path::PathBuf> {
    if root.exists() {
        return Err(Error::already_exists(root.display().to_string()));
    }
    let parent = root.parent().unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| Error::Infrastructure(error.to_string()))?;
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::infrastructure("index path has an invalid final component"))?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| Error::infrastructure(error.to_string()))?
        .as_nanos();
    let staging = parent.join(format!(
        ".{name}.searchkernel-creating-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&staging).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Error::already_exists(root.display().to_string())
        } else {
            Error::Infrastructure(error.to_string())
        }
    })?;
    Ok(staging)
}

#[cfg(feature = "native")]
pub(super) fn publish_staging_directory(
    staging: &std::path::Path,
    root: &std::path::Path,
) -> Result<()> {
    std::fs::rename(staging, root).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Error::already_exists(root.display().to_string())
        } else {
            Error::Infrastructure(error.to_string())
        }
    })?;
    let parent = root.parent().unwrap_or_else(|| std::path::Path::new("."));
    // The rename is the publication point, but create success also promises
    // that the new directory entry has reached the filesystem durability
    // boundary. If syncing the parent fails, the published root must remain
    // visible for recovery and the caller must be told that durability is
    // uncertain rather than being given ordinary success.
    sync_published_parent(parent).map_err(|error| {
        Error::search(
            crate::ErrorCode::Unavailable,
            format!(
                "index was published at '{}' but parent directory sync failed: {error}",
                root.display()
            ),
        )
        .with_detail(
            "index_published_durability_uncertain",
            Some(root.display().to_string()),
            "published index remains available for recovery but crash durability is uncertain",
        )
    })?;
    Ok(())
}

#[cfg(feature = "native")]
fn sync_published_parent(parent: &std::path::Path) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_PARENT_FSYNC.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other("injected parent fsync failure"));
    }
    std::fs::File::open(parent).and_then(|directory| directory.sync_all())
}

#[cfg(feature = "native")]
pub(super) fn remove_published_root(root: &std::path::Path) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_PUBLISHED_CLEANUP.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other(
            "injected published-root cleanup failure",
        ));
    }
    std::fs::remove_dir_all(root)
}

#[cfg(all(feature = "native", feature = "internal-api"))]
fn delete_tombstone_path(root: &std::path::Path) -> Result<std::path::PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TOMBSTONE: AtomicU64 = AtomicU64::new(0);

    let parent = root.parent().unwrap_or_else(|| std::path::Path::new("."));
    let name = root
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| Error::Infrastructure("index path has no valid final component".into()))?;
    loop {
        let sequence = NEXT_TOMBSTONE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{name}.searchkernel-delete-{}-{sequence}",
            std::process::id()
        ));
        match std::fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(candidate),
            Ok(_) => continue,
            Err(error) => return Err(Error::Infrastructure(error.to_string())),
        }
    }
}

#[cfg(feature = "native")]
fn open_native_backend(
    data: &std::path::Path,
    config: &std::path::Path,
) -> std::result::Result<searchkernel_ir_adapter::VespaBackend, searchkernel_ir::PortableError> {
    #[cfg(test)]
    if FAIL_NEXT_BACKEND_OPEN.with(|flag| flag.replace(false)) {
        return Err(searchkernel_ir::PortableError::new(
            searchkernel_ir::ErrorCode::Unavailable,
            "injected backend open failure",
        ));
    }
    searchkernel_ir_adapter::VespaBackend::open(data, config)
}

#[cfg(all(feature = "native", test))]
thread_local! {
    pub(super) static FAIL_NEXT_PARENT_FSYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static FAIL_NEXT_PUBLISHED_CLEANUP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static FAIL_NEXT_BACKEND_OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(feature = "native")]
pub(super) struct LifecycleLock {
    _file: std::fs::File,
}

#[cfg(feature = "native")]
impl LifecycleLock {
    fn acquire(root: &std::path::Path) -> Result<Self> {
        use std::os::fd::AsRawFd;

        let parent = root.parent().unwrap_or_else(|| std::path::Path::new("."));
        std::fs::create_dir_all(parent)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        let name = root
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Error::infrastructure("index path has an invalid final component"))?;
        let path = parent.join(format!(".{name}.searchkernel.lock"));
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        // This lock covers the interval in which a complete index is being
        // assembled and published, when its final data directory is absent.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = std::io::Error::last_os_error();
            if matches!(error.raw_os_error(), Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN)
            {
                return Err(Error::already_exists(root.display().to_string()));
            }
            return Err(Error::Infrastructure(format!(
                "could not lock index lifecycle path '{}': {error}",
                path.display()
            )));
        }
        Ok(Self { _file: file })
    }
}

#[cfg(all(feature = "native", test))]
pub(super) fn create_directory_transaction<T>(
    root: &std::path::Path,
    initialize: impl FnOnce(&std::path::Path) -> Result<T>,
) -> Result<T> {
    if root.exists() {
        return Err(Error::already_exists(root.display().to_string()));
    }
    if let Some(parent) = root.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
    }
    std::fs::create_dir(root).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Error::already_exists(root.display().to_string())
        } else {
            Error::Infrastructure(error.to_string())
        }
    })?;

    match initialize(root) {
        Ok(value) => Ok(value),
        Err(error) => {
            if let Err(cleanup_error) = std::fs::remove_dir_all(root) {
                return Err(Error::Infrastructure(format!(
                    "{error}; failed to clean up partial index {}: {cleanup_error}",
                    root.display()
                )));
            }
            Err(error)
        }
    }
}

#[cfg(feature = "native")]
impl Index {
    /// Validate persisted identity/configuration without opening Vespa or
    /// taking its lifetime lock. Daemon callers coordinate this inspection
    /// with create/delete using their existing per-target transition lock.
    #[doc(hidden)]
    #[cfg(feature = "internal-api")]
    pub fn inspect_storage(path: impl AsRef<std::path::Path>) -> Result<()> {
        let root = path.as_ref();
        fn directory(path: &std::path::Path) -> Result<()> {
            let metadata = std::fs::symlink_metadata(path)
                .map_err(|error| Error::Infrastructure(error.to_string()))?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(Error::corrupt_index("invalid persisted index directory"));
            }
            Ok(())
        }
        fn no_links(path: &std::path::Path) -> Result<()> {
            for entry in std::fs::read_dir(path)
                .map_err(|error| Error::Infrastructure(error.to_string()))? {
                let entry = entry.map_err(|error| Error::Infrastructure(error.to_string()))?;
                let kind = entry.file_type().map_err(|error| Error::Infrastructure(error.to_string()))?;
                if kind.is_symlink() || (!kind.is_file() && !kind.is_dir()) {
                    return Err(Error::corrupt_index("invalid persisted index metadata entry"));
                }
                if kind.is_dir() { no_links(&entry.path())?; }
            }
            Ok(())
        }
        directory(root)?;
        directory(&root.join("data"))?;
        for name in ["manifest", "config"] {
            directory(&root.join(name))?;
            no_links(&root.join(name))?;
        }
        let manifest = super::manifest::IndexManifest::read(root)?;
        manifest.resolved_context_with_config(&root.join("config"))?;
        Ok(())
    }

    /// Construct an index from the canonical FlatBuffers index-definition
    /// payload used by the native bootstrap. This is intentionally hidden from
    /// the ergonomic Rust API; the portable wire definition is validated by
    /// `searchkernel-ir` before it reaches the product lifecycle.
    #[doc(hidden)]
    #[cfg(feature = "internal-api")]
    pub fn create_from_definition_bytes(
        path: impl AsRef<std::path::Path>,
        bytes: &[u8],
    ) -> Result<Self> {
        let root = searchkernel_ir::__flatbuffers::root::<
            searchkernel_ir::wire::v1::schema::IndexDefinition,
        >(bytes)
        .map_err(|error| Error::Serialization(format!("invalid index definition: {error}")))?;
        let ir = searchkernel_ir::wire::v1::index_definition_to_ir(root).map_err(Error::from_ir)?;
        searchkernel_ir_adapter::validate_index_definition(&ir).map_err(Error::from_ir)?;
        let resolved_context = IndexContext::from_definition_ir(ir);
        Self::create_with_definitions(path.as_ref(), bytes.to_vec(), resolved_context)
    }

    /// Create a new on-disk index and persist all information required to
    /// reopen it safely.
    pub fn create(path: impl AsRef<std::path::Path>, definition: IndexDefinition) -> Result<Self> {
        let root = path.as_ref();
        let portable_definition_bytes =
            crate::portable_definition::try_encode_portable_definition(&definition)?;
        let resolved_ir = definition_to_ir(&definition)?;
        searchkernel_ir_adapter::validate_index_definition(&resolved_ir).map_err(Error::from_ir)?;
        let resolved_context = IndexContext::from_definition_ir(resolved_ir);
        Self::create_with_definitions(root, portable_definition_bytes, resolved_context)
    }

    fn create_with_definitions(
        root: &std::path::Path,
        portable_definition_bytes: Vec<u8>,
        resolved_context: IndexContext,
    ) -> Result<Self> {
        // Create and open can arrive in either order. If an opener briefly
        // owns the lock while observing an absent root, retrying here avoids
        // turning that harmless ordering into an AlreadyExists failure.
        let lifecycle_lock = loop {
            match LifecycleLock::acquire(root) {
                Ok(lock) => break lock,
                Err(error)
                    if error.code() == crate::error::ErrorCode::AlreadyExists && !root.is_dir() =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        };
        let staging = create_staging_directory(root)?;
        let initialize = (|| {
            let staging_data = staging.join("data");
            let staging_config = staging.join("config");
            std::fs::create_dir(&staging_data)
                .map_err(|error| Error::Infrastructure(error.to_string()))?;
            searchkernel_ir_adapter::compile_index_definition(
                resolved_context.definition(),
                &staging_config,
            )
            .map_err(Error::from_ir)?;
            let manifest = super::manifest::IndexManifest::for_definitions(
                &portable_definition_bytes,
                &resolved_context,
                &staging_config,
            )?;
            manifest.write(&staging)?;
            Ok::<(), Error>(())
        })();
        if let Err(error) = initialize {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
        // Publish the complete on-disk description before opening Vespa. The
        // native backend retains its data-directory path for asynchronous
        // flushes, so it must be opened at the final path rather than at the
        // temporary staging path.
        if let Err(error) = publish_staging_directory(&staging, root) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
        let backend = match open_native_backend(&root.join("data"), &root.join("config")) {
            Ok(backend) => backend,
            Err(error) => {
                return match remove_published_root(root) {
                    Ok(()) => Err(Error::Infrastructure(error.to_string())),
                    Err(cleanup_error) => Err(Error::search(
                        crate::ErrorCode::Unavailable,
                        format!("index was published but backend opening failed: {error}; cleanup failed: {cleanup_error}"),
                    )
                    .with_detail(
                        "index_published",
                        Some(root.display().to_string()),
                        "published index remains available for recovery",
                    )),
                };
            }
        };
        Self::from_owned_backend(
            resolved_context,
            BackendSession::new(Box::new(backend)),
            Some(lifecycle_lock),
        )
    }

    /// Reopen an existing index using its durable manifest.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let root = path.as_ref();
        // A creator holds the lifecycle lock while it assembles the staging
        // tree, before atomically publishing `root`. An opener that arrives in
        // that interval must wait for publication; reporting AlreadyExists
        // would turn a successful create/open race into two failures.
        let lifecycle_lock = loop {
            match LifecycleLock::acquire(root) {
                Ok(lock) => break lock,
                Err(error)
                    if error.code() == crate::error::ErrorCode::AlreadyExists && !root.is_dir() =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        };
        if !root.is_dir() {
            return Err(Error::not_found(root.display().to_string()));
        }
        let manifest = super::manifest::IndexManifest::read(root)?;
        let config = root.join("config");
        if !root.join("data").is_dir() || !config.is_dir() {
            return Err(Error::corrupt_index(
                "index data/config directories are missing",
            ));
        }
        let resolved_context = manifest.resolved_context_with_config(&config)?;
        let backend = open_native_backend(&root.join("data"), &config)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        Self::from_owned_backend(
            resolved_context,
            BackendSession::new(Box::new(backend)),
            Some(lifecycle_lock),
        )
    }

    /// Move one validated path-owned index into a daemon-owned storage path.
    ///
    /// This is local control-plane plumbing for the managed-local transition
    /// from legacy `--index PATH` ownership to canonical logical targets.
    /// Both lifecycle locks are held across validation and rename.
    #[doc(hidden)]
    #[cfg(feature = "internal-api")]
    pub fn migrate_storage(
        source: impl AsRef<std::path::Path>,
        destination: impl AsRef<std::path::Path>,
    ) -> Result<()> {
        use std::time::{Duration, Instant};

        const LOCK_WAIT: Duration = Duration::from_secs(5);
        const LOCK_RETRY: Duration = Duration::from_millis(10);

        fn unsafe_path(label: &str, path: &std::path::Path) -> Error {
            Error::search(
                crate::error::ErrorCode::FailedPrecondition,
                format!("unsafe {label} path: {}", path.display()),
            )
        }

        fn metadata_no_follow(
            label: &str,
            path: &std::path::Path,
        ) -> Result<Option<std::fs::Metadata>> {
            match std::fs::symlink_metadata(path) {
                Ok(metadata) if metadata.file_type().is_symlink() => Err(unsafe_path(label, path)),
                Ok(metadata) => Ok(Some(metadata)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(Error::Infrastructure(format!(
                    "inspect {label} path '{}': {error}",
                    path.display()
                ))),
            }
        }

        let source = source.as_ref();
        let destination = destination.as_ref();

        if let Some(metadata) = metadata_no_follow("managed migration destination", destination)? {
            if !metadata.is_dir() {
                return Err(unsafe_path("managed migration destination", destination));
            }
            if metadata_no_follow("legacy migration source", source)?.is_none() {
                return Ok(());
            }
            return Err(Error::already_exists(destination.display().to_string()));
        }
        if let Some(metadata) = metadata_no_follow("legacy migration source", source)? {
            if !metadata.is_dir() {
                return Err(unsafe_path("legacy migration source", source));
            }
        } else {
            return Err(Error::not_found(source.display().to_string()));
        }

        let source_deadline = Instant::now() + LOCK_WAIT;
        let _source_lock = loop {
            match LifecycleLock::acquire(source) {
                Ok(lock) => break lock,
                Err(error) if error.code() == crate::error::ErrorCode::AlreadyExists => {
                    if Instant::now() >= source_deadline {
                        return Err(Error::from_ir(searchkernel_ir::index_in_use_error()));
                    }
                    std::thread::sleep(LOCK_RETRY);
                }
                Err(error) => return Err(error),
            }
        };

        match metadata_no_follow("legacy migration source", source)? {
            Some(metadata) if metadata.is_dir() => {}
            Some(_) => return Err(unsafe_path("legacy migration source", source)),
            None => {
                if let Some(metadata) =
                    metadata_no_follow("managed migration destination", destination)?
                {
                    if metadata.is_dir() {
                        return Ok(());
                    }
                }
                return Err(Error::not_found(source.display().to_string()));
            }
        }

        let manifest = super::manifest::IndexManifest::read(source)?;
        let config = source.join("config");
        if !source.join("data").is_dir() || !config.is_dir() {
            return Err(Error::corrupt_index(
                "index data/config directories are missing",
            ));
        }
        manifest.validate_config(&config)?;

        let destination_parent = destination
            .parent()
            .ok_or_else(|| Error::infrastructure("managed index path has no parent directory"))?;
        std::fs::create_dir_all(destination_parent)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;

        let destination_deadline = Instant::now() + LOCK_WAIT;
        let _destination_lock = loop {
            match LifecycleLock::acquire(destination) {
                Ok(lock) => break lock,
                Err(error) if error.code() == crate::error::ErrorCode::AlreadyExists => {
                    if Instant::now() >= destination_deadline {
                        return Err(Error::from_ir(searchkernel_ir::index_in_use_error()));
                    }
                    std::thread::sleep(LOCK_RETRY);
                }
                Err(error) => return Err(error),
            }
        };

        match metadata_no_follow("managed migration destination", destination)? {
            None => {}
            Some(metadata) => {
                if !metadata.is_dir() {
                    return Err(unsafe_path("managed migration destination", destination));
                }
                if metadata_no_follow("legacy migration source", source)?.is_none() {
                    return Ok(());
                }
                return Err(Error::already_exists(destination.display().to_string()));
            }
        }

        std::fs::rename(source, destination).map_err(|error| {
            Error::Infrastructure(format!(
                "migrate legacy index '{}' to '{}': {error}",
                source.display(),
                destination.display()
            ))
        })?;

        let source_parent = source.parent().unwrap_or_else(|| std::path::Path::new("."));
        if let Err(error) = sync_published_parent(source_parent)
            .and_then(|_| sync_published_parent(destination_parent))
        {
            return Err(Error::search(
                crate::error::ErrorCode::Unavailable,
                format!(
                    "legacy index migrated to '{}' but parent directory sync failed: {error}",
                    destination.display()
                ),
            )
            .with_detail(
                "index_migration_durability_uncertain",
                None::<String>,
                "legacy index was moved but crash durability is uncertain",
            ));
        }

        Ok(())
    }

    /// Delete a persisted index after proving that no other process owns its
    /// lifecycle lock. This is daemon control-plane plumbing, not part of the
    /// ergonomic document API.
    #[doc(hidden)]
    #[cfg(feature = "internal-api")]
    pub fn delete_storage(path: impl AsRef<std::path::Path>) -> Result<()> {
        let root = path.as_ref();
        let _lifecycle_lock = LifecycleLock::acquire(root).map_err(|error| {
            if error.code() == crate::error::ErrorCode::AlreadyExists {
                Error::from_ir(searchkernel_ir::index_in_use_error())
            } else {
                error
            }
        })?;
        if !root.is_dir() {
            return Err(Error::not_found(root.display().to_string()));
        }

        let parent = root.parent().unwrap_or_else(|| std::path::Path::new("."));
        let tombstone = delete_tombstone_path(root)?;
        // Unpublish the complete tree in one rename. Recursive cleanup only
        // touches the hidden tombstone, so a failure cannot leave a partial
        // index visible at its canonical target path.
        std::fs::rename(root, &tombstone).map_err(|error| {
            Error::Infrastructure(format!("unpublish index '{}': {error}", root.display()))
        })?;
        sync_published_parent(parent).map_err(|error| {
            Error::search(
                crate::error::ErrorCode::Unavailable,
                format!(
                    "index was unpublished at '{}' but parent directory sync failed: {error}",
                    root.display()
                ),
            )
            .with_detail(
                "index_deleted_durability_uncertain",
                None::<String>,
                "index canonical path was unpublished but crash durability is uncertain",
            )
        })?;

        remove_published_root(&tombstone).map_err(|error| {
            Error::Infrastructure(format!(
                "index '{}' was deleted but tombstone cleanup failed: {error}",
                root.display()
            ))
        })
    }

    /// Construct an index around a backend for tests and infrastructure.
    #[doc(hidden)]
    pub fn from_backend(definition: IndexDefinition, backend: Arc<dyn Backend>) -> Result<Self> {
        let context = IndexContext::from_definition_ir(definition_to_ir(&definition)?);
        Self::from_owned_backend(context, BackendSession::from_backend(backend), None)
    }

    pub(super) fn from_owned_backend(
        context: IndexContext,
        session: Arc<BackendSession>,
        lifecycle_lock: Option<LifecycleLock>,
    ) -> Result<Self> {
        let effective_definition = definition_from_ir(context.definition());
        let context = Arc::new(context);
        let definition = Arc::new(effective_definition);
        Ok(Self {
            inner: Arc::new(IndexInner {
                session,
                context,
                definition,
                lifecycle_lock: std::sync::Mutex::new(lifecycle_lock),
            }),
        })
    }

    /// Gracefully close this index and all of its clones after draining active
    /// operations.
    pub fn close(&self) -> Result<()> {
        let result = self.inner.session.close().map_err(Error::from_ir);
        // Release the publication lock only after the backend has drained and
        // shut down. All cloned handles share this same mutex.
        let _ = self
            .inner
            .lifecycle_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        result
    }
}
