use searchkernel_core::api;
use searchkernel_core::*;

struct CompileOnlyBackend;

impl Backend for CompileOnlyBackend {
    fn insert(&self, _: ResolvedInsertRequest) -> Result<api::InsertResult> {
        unimplemented!()
    }

    fn upsert(&self, _: ResolvedUpsertRequest) -> Result<api::UpsertResult> {
        unimplemented!()
    }

    fn patch(&self, _: ResolvedPatchRequest) -> Result<api::PatchResult> {
        unimplemented!()
    }

    fn delete(&self, _: ResolvedDeleteRequest) -> Result<api::DeleteResult> {
        unimplemented!()
    }

    fn get(&self, _: ResolvedGetRequest) -> Result<api::GetResult> {
        unimplemented!()
    }

    fn search(&self, _: ResolvedSearchRequest) -> Result<api::SearchResult> {
        unimplemented!()
    }

    fn analyze(&self, _: ResolvedAnalyzeRequest) -> Result<api::AnalyzeResult> {
        unimplemented!()
    }
}

fn compile_contract() {
    let definition = api::IndexDefinition {
        fields: vec![api::FieldDefinition {
            name: "title".to_owned(),
            kind: api::FieldKind::Text as i32,
            ..Default::default()
        }],
        ..Default::default()
    };
    let semantics = IndexSemantics::new(&definition).expect("valid definition");
    let _resolved = semantics.resolve_search(&api::SearchRequest::default());
    let _description = semantics.describe();
    let _inspection = semantics.inspect_query(&api::InspectQueryRequest {
        request: Some(api::SearchRequest::default()),
    });
    let _: Option<CompileOnlyBackend> = None;
}

fn main() {
    let _ = compile_contract as fn();
}
