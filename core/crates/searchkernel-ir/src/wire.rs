//! Versioned wire adapters around the wire-agnostic Searchkernel IR.
//!
//! Generated FlatBuffers types are intentionally reachable only below a wire
//! major. Core model, backend, and resolver APIs remain version-free.

pub mod v1 {
    pub use searchkernel_protocol::wire::v1 as schema;

    pub use crate::api_to_ir::control_plane::{
        resolve_control_plane_request, resolve_index_target, ControlPlaneRequest,
        ControlPlaneResolutionError,
    };
    pub use crate::api_to_ir::{
        analyze_result_from_api, analyzer_to_ir, delete_result_from_api, field_to_ir,
        get_result_from_api, index_definition_to_ir, insert_result_from_api, patch_result_from_api,
        portable_error_from_api, search_result_from_api, update_result_from_api,
        upsert_result_from_api, WriteRequestRef,
    };
    pub use crate::ir_to_api::*;

    pub fn index_context_from_api(
        definition: schema::IndexDefinition<'_>,
    ) -> crate::Result<crate::IndexContext> {
        crate::IndexContext::from_api(definition)
    }

    pub fn insert_to_ir(
        context: &crate::IndexContext,
        request: schema::InsertRequest<'_>,
    ) -> crate::Result<crate::InsertRequestIr> {
        context.insert_to_ir(request)
    }

    pub fn upsert_to_ir(
        context: &crate::IndexContext,
        request: schema::UpsertRequest<'_>,
    ) -> crate::Result<crate::UpsertRequestIr> {
        context.upsert_to_ir(request)
    }

    pub fn patch_to_ir(
        context: &crate::IndexContext,
        request: schema::PatchRequest<'_>,
    ) -> crate::Result<crate::PatchRequestIr> {
        context.patch_to_ir(request)
    }

    pub fn update_to_ir(
        context: &crate::IndexContext,
        request: schema::UpdateRequest<'_>,
    ) -> crate::Result<crate::UpdateRequestIr> {
        context.update_to_ir(request)
    }

    pub fn delete_to_ir(
        context: &crate::IndexContext,
        request: schema::DeleteRequest<'_>,
    ) -> crate::Result<crate::DeleteRequestIr> {
        context.delete_to_ir(request)
    }

    pub fn get_to_ir(
        context: &crate::IndexContext,
        request: schema::GetRequest<'_>,
    ) -> crate::Result<crate::GetRequestIr> {
        context.get_to_ir(request)
    }

    pub fn value_to_ir(
        context: &crate::IndexContext,
        field: &str,
        value: schema::Value<'_>,
    ) -> crate::Result<crate::FieldValueIr> {
        context.value_to_ir(field, value)
    }

    pub fn document_to_ir(
        context: &crate::IndexContext,
        document: schema::Document<'_>,
    ) -> crate::Result<crate::DocumentIr> {
        context.document_to_ir(document)
    }

    pub fn write_to_ir(
        context: &crate::IndexContext,
        request: WriteRequestRef<'_>,
    ) -> crate::Result<crate::WriteRequestIr> {
        context.write_to_ir(request)
    }

    pub fn bulk_write_to_ir<'a>(
        context: &crate::IndexContext,
        requests: impl IntoIterator<Item = WriteRequestRef<'a>>,
    ) -> crate::Result<Vec<crate::WriteRequestIr>> {
        context.bulk_write_to_ir(requests)
    }

    pub fn filter_to_ir(
        context: &crate::IndexContext,
        filter: schema::FilterExpr<'_>,
    ) -> crate::Result<crate::FilterIr> {
        context.filter_to_ir(filter)
    }

    pub fn comparison_to_ir(
        context: &crate::IndexContext,
        comparison: schema::FilterComparison<'_>,
    ) -> crate::Result<crate::ComparisonIr> {
        context.comparison_to_ir(comparison)
    }

    pub fn range_to_ir(
        context: &crate::IndexContext,
        range: schema::FilterRange<'_>,
    ) -> crate::Result<crate::RangeIr> {
        context.range_to_ir(range)
    }

    pub fn inspect_query(
        context: &crate::IndexContext,
        request: schema::SearchRequest<'_>,
    ) -> crate::Result<crate::SearchRequestIr> {
        context.inspect_query(request)
    }

    pub fn search_to_ir(
        context: &crate::IndexContext,
        request: schema::SearchRequest<'_>,
    ) -> crate::Result<crate::SearchRequestIr> {
        context.search_to_ir(request)
    }

    pub fn analyze_to_ir(
        context: &crate::IndexContext,
        request: schema::AnalyzeRequest<'_>,
    ) -> crate::Result<crate::AnalyzeRequestIr> {
        context.analyze_to_ir(request)
    }

    pub fn search_fields_to_ir(
        context: &crate::IndexContext,
        selection: schema::SearchFieldSelection<'_>,
    ) -> crate::Result<Vec<crate::SearchFieldIr>> {
        context.search_fields_to_ir(selection)
    }

    pub fn query_to_ir(
        context: &crate::IndexContext,
        text: &str,
        expression: Option<schema::QueryExpr<'_>>,
    ) -> crate::Result<crate::QueryIr> {
        context.query_to_ir(text, expression)
    }

    pub fn sort_to_ir(
        context: &crate::IndexContext,
        sort: schema::SortExpr<'_>,
    ) -> crate::Result<crate::SortIr> {
        context.sort_to_ir(sort)
    }

    pub fn projection_to_ir(
        context: &crate::IndexContext,
        projection: Option<schema::FieldProjection<'_>>,
    ) -> crate::Result<crate::ProjectionIr> {
        context.projection_to_ir(projection)
    }

    pub fn highlighting_to_ir(
        context: &crate::IndexContext,
        highlighting: Option<schema::HighlightRequest<'_>>,
    ) -> crate::Result<crate::HighlightingIr> {
        context.highlighting_to_ir(highlighting)
    }

    pub fn ranking_request_to_ir(
        context: &crate::IndexContext,
        ranking: Option<schema::RankingRequest<'_>>,
    ) -> crate::Result<crate::RankingRequestIr> {
        context.ranking_request_to_ir(ranking)
    }
}
