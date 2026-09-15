use searchkernel_ir::api;
use searchkernel_ir::*;

struct CompileOnlyBackend;

impl Backend for CompileOnlyBackend {
    fn insert(&self, _: InsertRequestIr) -> Result<InsertResult> {
        unimplemented!()
    }
    fn upsert(&self, _: UpsertRequestIr) -> Result<UpsertResult> {
        unimplemented!()
    }
    fn patch(&self, _: PatchRequestIr) -> Result<PatchResult> {
        unimplemented!()
    }
    fn delete(&self, _: DeleteRequestIr) -> Result<DeleteResult> {
        unimplemented!()
    }
    fn get(&self, _: GetRequestIr) -> Result<GetResult> {
        unimplemented!()
    }
    fn search(&self, _: SearchRequestIr) -> Result<SearchResult> {
        unimplemented!()
    }
    fn analyze(&self, _: AnalyzeRequestIr) -> Result<AnalyzeResult> {
        unimplemented!()
    }
}

fn compile_contract<'a>(definition: api::IndexDefinition<'a>, request: api::SearchRequest<'a>) {
    let context = IndexContext::from_api(definition).expect("valid definition");
    let _request_ir = context.search_to_ir(request);
    let _description = context.describe();
    let _inspection = context.inspect_query(request);
    let _ = context.definition();
    let _: Option<CompileOnlyBackend> = None;
}

fn main() {
    let _ = compile_contract as for<'a> fn(api::IndexDefinition<'a>, api::SearchRequest<'a>);
}
