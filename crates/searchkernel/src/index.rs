use crate::document::{serialize, DocumentId};
use crate::error::{Error, Result};
use crate::patch::Patch;
use crate::result;
use crate::schema::IndexDefinition;
use crate::search::SearchBuilder;
use crate::wire;
use searchkernel_core::{api, Backend, IndexContext};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;

pub struct Index {
    pub(crate) context: Arc<IndexContext>,
    pub(crate) backend: Arc<dyn Backend>,
}

impl Clone for Index {
    fn clone(&self) -> Self {
        Self {
            context: Arc::clone(&self.context),
            backend: Arc::clone(&self.backend),
        }
    }
}

impl Index {
    /// Construct an index around a backend for tests and infrastructure.
    ///
    /// This is hidden from normal API documentation because production
    /// lifecycle is supplied by a later integration milestone.
    #[doc(hidden)]
    pub fn from_backend(definition: IndexDefinition, backend: Arc<dyn Backend>) -> Result<Self> {
        let bytes = wire::definition(&definition);
        let root = searchkernel_core::__flatbuffers::root::<api::IndexDefinition>(&bytes).map_err(
            |error| Error::infrastructure(format!("invalid generated index definition: {error}")),
        )?;
        let context = IndexContext::from_api(root)?;
        Ok(Self {
            context: Arc::new(context),
            backend,
        })
    }

    pub fn insert<T: Serialize>(&self, id: impl Into<DocumentId>, document: &T) -> Result<()> {
        let id = id.into();
        let value = serialize(document).map_err(|error| Error::Serialization(error.to_string()))?;
        let bytes = wire::insert(id.as_str(), &value)?;
        let request = self.root_insert(&bytes)?;
        let _result = self.backend.insert(self.context.insert_to_ir(request)?)?;
        Ok(())
    }

    pub fn upsert<T: Serialize>(&self, id: impl Into<DocumentId>, document: &T) -> Result<()> {
        let id = id.into();
        let value = serialize(document).map_err(|error| Error::Serialization(error.to_string()))?;
        let bytes = wire::upsert(id.as_str(), &value)?;
        let request = self.root_upsert(&bytes)?;
        let _result = self.backend.upsert(self.context.upsert_to_ir(request)?)?;
        Ok(())
    }

    pub fn patch(&self, id: impl Into<DocumentId>, patch: Patch) -> Result<()> {
        let id = id.into();
        let bytes = wire::patch(id.as_str(), &patch)?;
        let request = self.root_patch(&bytes)?;
        let _result = self.backend.patch(self.context.patch_to_ir(request)?)?;
        Ok(())
    }

    pub fn delete(&self, id: impl Into<DocumentId>) -> Result<bool> {
        let id = id.into();
        let bytes = wire::delete(id.as_str());
        let request = self.root_delete(&bytes)?;
        Ok(self
            .backend
            .delete(self.context.delete_to_ir(request)?)?
            .deleted)
    }

    pub fn get<T: DeserializeOwned>(&self, id: impl Into<DocumentId>) -> Result<Option<T>> {
        let id = id.into();
        let bytes = wire::get(id.as_str());
        let request = self.root_get(&bytes)?;
        let result = self.backend.get(self.context.get_to_ir(request)?)?;
        result
            .document
            .map(result::document)
            .transpose()
            .map(|value| value.map(|(_, document)| document))
    }

    pub fn search(&self, query: impl Into<String>) -> SearchBuilder<'_> {
        SearchBuilder::new(self, query)
    }

    pub(crate) fn execute_search<T: DeserializeOwned>(
        &self,
        request: &SearchBuilder<'_>,
    ) -> Result<result::SearchResult<T>> {
        let bytes = wire::search(request)?;
        let request = searchkernel_core::__flatbuffers::root::<api::SearchRequest>(&bytes)
            .map_err(|error| {
                Error::infrastructure(format!("invalid generated search request: {error}"))
            })?;
        let request = self.context.search_to_ir(request)?;
        let result = self.backend.search(request)?;
        result::search_result(result)
    }

    fn root_insert<'a>(&self, bytes: &'a [u8]) -> Result<api::InsertRequest<'a>> {
        searchkernel_core::__flatbuffers::root::<api::InsertRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated insert request: {error}"))
        })
    }

    fn root_upsert<'a>(&self, bytes: &'a [u8]) -> Result<api::UpsertRequest<'a>> {
        searchkernel_core::__flatbuffers::root::<api::UpsertRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated upsert request: {error}"))
        })
    }

    fn root_patch<'a>(&self, bytes: &'a [u8]) -> Result<api::PatchRequest<'a>> {
        searchkernel_core::__flatbuffers::root::<api::PatchRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated patch request: {error}"))
        })
    }

    fn root_delete<'a>(&self, bytes: &'a [u8]) -> Result<api::DeleteRequest<'a>> {
        searchkernel_core::__flatbuffers::root::<api::DeleteRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated delete request: {error}"))
        })
    }

    fn root_get<'a>(&self, bytes: &'a [u8]) -> Result<api::GetRequest<'a>> {
        searchkernel_core::__flatbuffers::root::<api::GetRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated get request: {error}"))
        })
    }
}
