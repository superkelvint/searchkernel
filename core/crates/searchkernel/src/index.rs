use crate::document::{serialize, DocumentId};
use crate::error::{Error, Result};
use crate::patch::Patch;
use crate::result;
use crate::schema::IndexDefinition;
use crate::search::SearchBuilder;
use crate::wire;
use searchkernel_ir::{api, Backend, IndexContext};
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
        let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes).map_err(
            |error| Error::infrastructure(format!("invalid generated index definition: {error}")),
        )?;
        let context = IndexContext::from_api(root)?;
        Ok(Self {
            context: Arc::new(context),
            backend,
        })
    }

    /// Insert a new document.
    pub fn insert<T: Serialize>(&self, id: impl Into<DocumentId>, document: &T) -> Result<()> {
        let document_id = id.into();
        let document_value =
            serialize(document).map_err(|error| Error::Serialization(error.to_string()))?;
        let request_bytes = wire::insert(document_id.as_str(), &document_value)?;
        let request = self.root_insert(&request_bytes)?;
        let ir_request = self.context.insert_to_ir(request)?;
        self.backend.insert(ir_request)?;
        Ok(())
    }

    /// Insert a document or replace the existing document with the same ID.
    pub fn upsert<T: Serialize>(&self, id: impl Into<DocumentId>, document: &T) -> Result<()> {
        let document_id = id.into();
        let document_value =
            serialize(document).map_err(|error| Error::Serialization(error.to_string()))?;
        let request_bytes = wire::upsert(document_id.as_str(), &document_value)?;
        let request = self.root_upsert(&request_bytes)?;
        let ir_request = self.context.upsert_to_ir(request)?;
        self.backend.upsert(ir_request)?;
        Ok(())
    }

    /// Apply field updates to an existing document.
    pub fn patch(&self, id: impl Into<DocumentId>, patch: Patch) -> Result<()> {
        let document_id = id.into();
        let request_bytes = wire::patch(document_id.as_str(), &patch)?;
        let request = self.root_patch(&request_bytes)?;
        let ir_request = self.context.patch_to_ir(request)?;
        self.backend.patch(ir_request)?;
        Ok(())
    }

    /// Delete a document and return whether it existed.
    pub fn delete(&self, id: impl Into<DocumentId>) -> Result<bool> {
        let document_id = id.into();
        let request_bytes = wire::delete(document_id.as_str());
        let request = self.root_delete(&request_bytes)?;
        let ir_request = self.context.delete_to_ir(request)?;
        let delete_result = self.backend.delete(ir_request)?;
        Ok(delete_result.deleted)
    }

    /// Read a document and deserialize it into `T`.
    pub fn get<T: DeserializeOwned>(&self, id: impl Into<DocumentId>) -> Result<Option<T>> {
        let document_id = id.into();
        let request_bytes = wire::get(document_id.as_str());
        let request = self.root_get(&request_bytes)?;
        let get_result = self.backend.get(self.context.get_to_ir(request)?)?;
        get_result
            .document
            .map(result::document)
            .transpose()
            .map(|value| value.map(|(_, document)| document))
    }

    /// Start a search for `query`.
    pub fn search(&self, query: impl Into<String>) -> SearchBuilder<'_> {
        SearchBuilder::new(self, query)
    }

    pub(crate) fn execute_search<T: DeserializeOwned>(
        &self,
        request: &SearchBuilder<'_>,
    ) -> Result<result::SearchResult<T>> {
        let request_bytes = wire::search(request)?;
        let wire_request =
            searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&request_bytes).map_err(
                |error| Error::infrastructure(format!("invalid generated search request: {error}")),
            )?;
        let ir_request = self.context.search_to_ir(wire_request)?;
        let backend_result = self.backend.search(ir_request)?;
        result::search_result(backend_result)
    }

    fn root_insert<'a>(&self, bytes: &'a [u8]) -> Result<api::InsertRequest<'a>> {
        searchkernel_ir::__flatbuffers::root::<api::InsertRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated insert request: {error}"))
        })
    }

    fn root_upsert<'a>(&self, bytes: &'a [u8]) -> Result<api::UpsertRequest<'a>> {
        searchkernel_ir::__flatbuffers::root::<api::UpsertRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated upsert request: {error}"))
        })
    }

    fn root_patch<'a>(&self, bytes: &'a [u8]) -> Result<api::PatchRequest<'a>> {
        searchkernel_ir::__flatbuffers::root::<api::PatchRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated patch request: {error}"))
        })
    }

    fn root_delete<'a>(&self, bytes: &'a [u8]) -> Result<api::DeleteRequest<'a>> {
        searchkernel_ir::__flatbuffers::root::<api::DeleteRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated delete request: {error}"))
        })
    }

    fn root_get<'a>(&self, bytes: &'a [u8]) -> Result<api::GetRequest<'a>> {
        searchkernel_ir::__flatbuffers::root::<api::GetRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated get request: {error}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Analyzer, Field, SearchField};
    use searchkernel_ir::{AnalyzerIdentity, FieldCardinality, FieldKind};
    use searchkernel_testkit::RecordingBackend;

    #[test]
    fn schema_options_and_default_search_fields_reach_core() {
        let definition = IndexDefinition::new()
            .name("catalog")
            .field(
                Field::text("title")
                    .stored(false)
                    .searchable(true)
                    .filterable(false)
                    .sortable(false)
                    .facetable(false)
                    .analyzer(Analyzer::standard_v1()),
            )
            .field(
                Field::keyword("brand")
                    .stored(false)
                    .searchable(false)
                    .filterable(true)
                    .sortable(true)
                    .facetable(true),
            )
            .field(
                Field::int64("quantity")
                    .stored(true)
                    .searchable(false)
                    .filterable(true)
                    .sortable(true)
                    .facetable(true),
            )
            .field(
                Field::float64("price")
                    .stored(false)
                    .searchable(false)
                    .filterable(true)
                    .sortable(false)
                    .facetable(false),
            )
            .field(
                Field::bool("enabled")
                    .multi()
                    .stored(true)
                    .searchable(false)
                    .filterable(true)
                    .sortable(false)
                    .facetable(true),
            )
            .default_search_fields([SearchField::new("title").weight(0.0)]);
        let index = Index::from_backend(definition, Arc::new(RecordingBackend::new())).unwrap();
        let description = index.context.describe();

        assert_eq!(description.name().unwrap().as_str(), "catalog");
        assert_eq!(description.fields().len(), 5);
        assert_eq!(description.default_search_fields().len(), 1);
        assert_eq!(description.default_search_fields()[0].weight().get(), 0.0);

        let fields = description.fields();
        assert_eq!(fields[0].name().as_str(), "title");
        assert_eq!(fields[0].kind(), FieldKind::Text);
        assert_eq!(fields[0].cardinality(), FieldCardinality::Single);
        assert_eq!(fields[0].analyzer(), Some(AnalyzerIdentity::StandardV1));
        assert!(!fields[0].capabilities().stored());
        assert!(fields[0].capabilities().searchable());

        assert_eq!(fields[1].kind(), FieldKind::Keyword);
        assert!(!fields[1].capabilities().stored());
        assert!(fields[1].capabilities().filterable());
        assert!(fields[1].capabilities().sortable());
        assert!(fields[1].capabilities().facetable());

        assert_eq!(fields[2].kind(), FieldKind::Int64);
        assert!(fields[2].capabilities().facetable());
        assert_eq!(fields[3].kind(), FieldKind::Float64);
        assert!(!fields[3].capabilities().stored());
        assert_eq!(fields[4].kind(), FieldKind::Bool);
        assert_eq!(fields[4].cardinality(), FieldCardinality::Multi);
        assert!(!fields[4].capabilities().sortable());
    }
}
