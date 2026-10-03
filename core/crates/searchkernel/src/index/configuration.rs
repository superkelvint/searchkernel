//! Rank configuration publication. A durable backup is the rollback journal;
//! reopen restores it after interrupted publication. Engine data is never moved.
use super::{manifest::IndexManifest, native::open_native_backend, Index};
use crate::{Error, Result};
use searchkernel_ir::{BackendSession, IndexContext};
use std::{fs, path::Path};

const FILES: [&str; 3] = [
    "portable-index-definition.bfbs",
    "index-definition.bfbs",
    "index.json",
];
const BACKUP: &str = ".configuration-backup";
const NEXT: &str = ".configuration-next";

fn io(error: std::io::Error) -> Error {
    Error::Infrastructure(error.to_string())
}
fn sync(path: &Path) -> Result<()> {
    fs::File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(io)
}
fn remove(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(io)?;
    }
    Ok(())
}
fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir(destination).map_err(io)?;
    for entry in fs::read_dir(source).map_err(io)? {
        let entry = entry.map_err(io)?;
        let target = destination.join(entry.file_name());
        let kind = entry.file_type().map_err(io)?;
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target).map_err(io)?;
            sync(&target)?;
        } else {
            return Err(Error::corrupt_index(
                "configuration contains non-regular files",
            ));
        }
    }
    sync(destination)
}
fn copy_metadata(source: &Path, destination: &Path) -> Result<()> {
    let source = source.join("manifest");
    let destination = destination.join("manifest");
    fs::create_dir_all(&destination).map_err(io)?;
    for name in FILES {
        let pending = destination.join(format!("{name}.configuration-pending"));
        fs::copy(source.join(name), &pending).map_err(io)?;
        sync(&pending)?;
        fs::rename(pending, destination.join(name)).map_err(io)?;
    }
    sync(&destination)
}
fn install_config(source: &Path, root: &Path) -> Result<()> {
    let restore = root.join(".configuration-install");
    remove(&restore)?;
    copy_tree(&source.join("config"), &restore)?;
    let retired = root.join(".configuration-retired");
    remove(&retired)?;
    if root.join("config").exists() {
        fs::rename(root.join("config"), &retired).map_err(io)?;
    }
    fs::rename(restore, root.join("config")).map_err(io)?;
    sync(root)?;
    copy_metadata(source, root)
}

pub(super) fn recover(root: &Path) -> Result<()> {
    let backup = root.join(BACKUP);
    if backup.exists() {
        // Copy, rather than consume, the journal so recovery is itself restartable.
        install_config(&backup, root)?;
        let committed = root.join(".configuration-committed");
        remove(&committed)?;
        fs::rename(&backup, &committed).map_err(io)?;
        sync(root)?;
    }
    for name in [
        NEXT,
        ".configuration-backup-building",
        ".configuration-committed",
        ".configuration-retired",
        ".configuration-install",
    ] {
        remove(&root.join(name))?;
    }
    Ok(())
}

impl Index {
    /// Apply a validated rank-profile-only definition and return its reopened
    /// owner. Existing clones become closed. The lifecycle lock covers shutdown,
    /// publication and reopen, including rollback; document storage is retained.
    pub fn apply_rank_definition_bytes(&self, bytes: &[u8]) -> Result<Self> {
        self.apply_rank_definition_bytes_inner(bytes)
            .map_err(|(error, _recovered)| error)
    }

    /// Internal daemon variant that returns the reopened rollback owner when
    /// publication fails after the active engine has been closed.
    #[doc(hidden)]
    #[cfg(feature = "internal-api")]
    pub fn apply_rank_definition_bytes_recovering(
        &self,
        bytes: &[u8],
    ) -> std::result::Result<Self, (Error, Option<Self>)> {
        self.apply_rank_definition_bytes_inner(bytes)
    }

    fn apply_rank_definition_bytes_inner(
        &self,
        bytes: &[u8],
    ) -> std::result::Result<Self, (Error, Option<Self>)> {
        let view = searchkernel_ir::__flatbuffers::root::<
            searchkernel_ir::wire::v1::schema::IndexDefinition,
        >(bytes)
        .map_err(|error| (Error::Serialization(error.to_string()), None))?;
        let mut definition = searchkernel_ir::wire::v1::index_definition_to_ir(view)
            .map_err(|error| (Error::from_ir(error), None))?;
        searchkernel_ir_adapter::validate_index_definition(&definition)
            .map_err(|error| (Error::from_ir(error), None))?;
        if !searchkernel_ir_adapter::rank_profile_change_supported(
            self.inner.context.definition(),
            &definition,
        ) {
            return Err((
                Error::search(
                    crate::ErrorCode::Unsupported,
                    "schema changes require reindexing",
                ),
                None,
            ));
        }
        definition
            .set_resolved_document_type(self.inner.context.definition().resolved_document_type());
        let mut lock = self
            .inner
            .lifecycle_lock
            .lock()
            .map_err(|_| (Error::index_closed(), None))?;
        let root = lock
            .as_ref()
            .ok_or_else(|| (Error::index_closed(), None))?
            .root
            .clone();
        recover(&root).map_err(|error| (error, None))?;
        let next = root.join(NEXT);
        fs::create_dir(&next).map_err(|error| (io(error), None))?;
        let prepare = (|| {
            let context = IndexContext::from_definition_ir(definition);
            searchkernel_ir_adapter::compile_index_definition(
                context.definition(),
                next.join("config"),
            )
            .map_err(Error::from_ir)?;
            IndexManifest::for_definitions(bytes, &context, &next.join("config"))?.write(&next)?;
            // Compile ranking expressions through Vespa before closing the active
            // engine; configuration validation alone does not compile expressions.
            let probe = open_native_backend(&next.join("probe-data"), &next.join("config"))
                .map_err(|error| Error::Infrastructure(error.to_string()))?;
            drop(probe);
            let building = root.join(".configuration-backup-building");
            fs::create_dir(&building).map_err(io)?;
            copy_tree(&root.join("config"), &building.join("config"))?;
            copy_metadata(&root, &building)?;
            fs::rename(&building, root.join(BACKUP)).map_err(io)?;
            sync(&root)?;
            Ok(context)
        })();
        let context = match prepare {
            Ok(context) => context,
            Err(error) => {
                if let Err(recovery_error) = recover(&root) {
                    return Err((recovery_error, None));
                }
                return Err((error, None));
            }
        };
        self.inner
            .session
            .close()
            .map_err(|error| (Error::from_ir(error), None))?;
        let publish = (|| {
            install_config(&next, &root)?;
            #[cfg(test)]
            if FAIL_AFTER_INSTALL.with(|flag| flag.replace(false)) {
                return Err(Error::Infrastructure(
                    "injected configuration reopen failure".to_owned(),
                ));
            }
            let backend = open_native_backend(&root.join("data"), &root.join("config"))
                .map_err(|error| Error::Infrastructure(error.to_string()))?;
            fs::rename(root.join(BACKUP), root.join(".configuration-committed")).map_err(io)?;
            if let Err(error) = sync(&root) {
                fs::rename(root.join(".configuration-committed"), root.join(BACKUP)).map_err(io)?;
                return Err(error);
            }
            Ok(backend)
        })();
        match publish {
            Ok(backend) => {
                // Publication is committed; leftover cleanup can be completed
                // on the next open without changing the accepted configuration.
                let owner = Self::from_owned_backend(
                    context,
                    BackendSession::new(Box::new(backend)),
                    lock.take(),
                )
                .map_err(|error| (error, None))?;
                let _ = recover(&root);
                Ok(owner)
            }
            Err(error) => {
                if let Err(recovery_error) = recover(&root) {
                    // Keep the lifecycle lock attached to the now-closed original
                    // owner until its registry entry is retired.
                    return Err((recovery_error, None));
                }

                // Reopen the restored configuration while still owning the same
                // filesystem lifecycle lock. This closes the cross-process gap
                // between rollback and registry publication.
                let restored = (|| {
                    let manifest = IndexManifest::read(&root)?;
                    let config = root.join("config");
                    let restored_context = manifest.resolved_context_with_config(&config)?;
                    let backend = open_native_backend(&root.join("data"), &config)
                        .map_err(|open_error| Error::Infrastructure(open_error.to_string()))?;
                    Self::from_owned_backend(
                        restored_context,
                        BackendSession::new(Box::new(backend)),
                        lock.take(),
                    )
                })();

                match restored {
                    Ok(owner) => Err((error, Some(owner))),
                    Err(reopen_error) => Err((
                        Error::search(
                            crate::ErrorCode::Unavailable,
                            format!(
                                "{error}; rollback restored persisted configuration but reopen failed: {reopen_error}"
                            ),
                        ),
                        None,
                    )),
                }
            }
        }
    }
}

#[cfg(test)]
thread_local! { static FAIL_AFTER_INSTALL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Field, IndexDefinition, RankProfile};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);
    struct Root(std::path::PathBuf);
    impl Root {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "rank-configuration-{}-{}",
                std::process::id(),
                NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn definition(expression: &str) -> IndexDefinition {
        IndexDefinition::new()
            .name("products")
            .field(Field::text("title").stored(true).searchable(true))
            .rank_profile(RankProfile::new("custom").first_phase(expression))
    }
    fn input(expression: &str) -> Vec<u8> {
        crate::portable_definition::try_encode_portable_definition(&definition(expression)).unwrap()
    }
    #[test]
    fn failed_configuration_reopen_restores_manifest_config_and_documents() {
        let root = Root::new();
        let index = Index::create(&root.0, definition("1")).unwrap();
        index
            .upsert("one", &serde_json::json!({"title":"orchid"}))
            .unwrap();
        FAIL_AFTER_INSTALL.with(|flag| flag.set(true));
        assert!(index.apply_rank_definition_bytes(&input("2")).is_err());
        let restored = Index::open(&root.0).unwrap();
        assert_eq!(
            restored.describe().unwrap().rank_profiles()[0]
                .first_phase
                .as_ref()
                .and_then(|phase| phase.expression.as_deref()),
            Some("1")
        );
        assert!(restored.get::<serde_json::Value>("one").unwrap().is_some());
        restored.close().unwrap();
    }
    #[test]
    fn invalid_rank_preparation_keeps_the_original_owner_open() {
        let root = Root::new();
        let index = Index::create(&root.0, definition("1")).unwrap();
        index
            .upsert("one", &serde_json::json!({"title":"orchid"}))
            .unwrap();
        assert!(index
            .apply_rank_definition_bytes(&input("invalid_function("))
            .is_err());
        assert_eq!(
            index.describe().unwrap().rank_profiles()[0]
                .first_phase
                .as_ref()
                .and_then(|phase| phase.expression.as_deref()),
            Some("1")
        );
        assert!(index.get::<serde_json::Value>("one").unwrap().is_some());
        assert!(!root.0.join(BACKUP).exists());
        index.close().unwrap();
    }

    #[test]
    fn interrupted_publication_is_restored_before_engine_open() {
        let root = Root::new();
        let index = Index::create(&root.0, definition("1")).unwrap();
        index
            .upsert("one", &serde_json::json!({"title":"orchid"}))
            .unwrap();
        index.close().unwrap();
        let backup = root.0.join(BACKUP);
        fs::create_dir(&backup).unwrap();
        copy_tree(&root.0.join("config"), &backup.join("config")).unwrap();
        copy_metadata(&root.0, &backup).unwrap();
        sync(&root.0).unwrap();
        fs::remove_dir_all(root.0.join("config")).unwrap();
        fs::write(root.0.join("manifest/index.json"), b"partial publication").unwrap();
        let restored = Index::open(&root.0).unwrap();
        assert_eq!(
            restored.describe().unwrap().rank_profiles()[0]
                .first_phase
                .as_ref()
                .and_then(|phase| phase.expression.as_deref()),
            Some("1")
        );
        assert!(restored.get::<serde_json::Value>("one").unwrap().is_some());
        assert!(!backup.exists());
        restored.close().unwrap();
    }
}
