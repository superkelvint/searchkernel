use crate::error::Result;
use crate::model::{
    AnalyzeRequestIr, AnalyzeResult, DeleteRequestIr, DeleteResult, GetRequestIr, GetResult,
    InsertRequestIr, InsertResult, PatchRequestIr, PatchResult, SearchRequestIr, SearchResult,
    UpsertRequestIr, UpsertResult,
};

/// Execution surface for a Search Kernel backend.
///
/// All requests have already crossed the API-to-IR boundary. A backend may
/// execute them, but it must not choose Search Kernel defaults or reinterpret
/// wire representation details.
pub trait Backend: Send + Sync {
    fn insert(&self, request: InsertRequestIr) -> Result<InsertResult>;
    fn upsert(&self, request: UpsertRequestIr) -> Result<UpsertResult>;
    fn patch(&self, request: PatchRequestIr) -> Result<PatchResult>;
    fn delete(&self, request: DeleteRequestIr) -> Result<DeleteResult>;
    fn get(&self, request: GetRequestIr) -> Result<GetResult>;
    fn search(&self, request: SearchRequestIr) -> Result<SearchResult>;
    fn analyze(&self, request: AnalyzeRequestIr) -> Result<AnalyzeResult>;

    /// Gracefully release backend-owned resources.
    ///
    /// Backends that have no explicit shutdown operation may use the default,
    /// which simply allows the backend value to be dropped.
    fn close(self: Box<Self>) -> Result<()> {
        Ok(())
    }
}
