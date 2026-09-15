use searchkernel::{Field, Index, IndexDefinition, Patch};
use searchkernel_ir::{
    Backend, DeleteRequestIr, DeleteResult, DocumentResult, FieldValueIr, GetRequestIr, GetResult,
    InsertRequestIr, InsertResult, PatchOperationIr, PatchRequestIr, PatchResult, SearchRequestIr,
    SearchResult, UpsertRequestIr, UpsertResult, Value,
};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct Product {
    title: String,
    brand: String,
    price: f64,
    in_stock: bool,
}

fn definition() -> IndexDefinition {
    IndexDefinition::new()
        .field(Field::text("title"))
        .field(Field::keyword("brand"))
        .field(Field::float64("price"))
        .field(Field::bool("in_stock"))
}

#[derive(Debug, Default)]
struct StatefulBackend {
    documents: Mutex<BTreeMap<String, BTreeMap<String, FieldValueIr>>>,
}

impl StatefulBackend {
    fn store(&self, document: &searchkernel_ir::DocumentIr) {
        self.documents.lock().unwrap().insert(
            document.id().as_str().to_owned(),
            document
                .fields()
                .iter()
                .map(|(field, value)| (field.as_str().to_owned(), value.clone()))
                .collect(),
        );
    }

    fn fields_to_result(fields: &BTreeMap<String, FieldValueIr>) -> BTreeMap<String, Value> {
        fields
            .iter()
            .map(|(field, value)| (field.clone(), field_value(value)))
            .collect()
    }
}

fn scalar_value(value: &searchkernel_ir::ScalarValueIr) -> Value {
    match value {
        searchkernel_ir::ScalarValueIr::Bool(value) => Value::Bool(*value),
        searchkernel_ir::ScalarValueIr::Int64(value) => Value::Int64(*value),
        searchkernel_ir::ScalarValueIr::Float64(value) => Value::Float64(*value),
        searchkernel_ir::ScalarValueIr::String(value) => Value::String(value.clone()),
    }
}

fn field_value(value: &FieldValueIr) -> Value {
    match value {
        FieldValueIr::Single(value) => scalar_value(value),
        FieldValueIr::Multi(values) => Value::List(values.iter().map(scalar_value).collect()),
    }
}

impl Backend for StatefulBackend {
    fn insert(&self, request: InsertRequestIr) -> searchkernel_ir::Result<InsertResult> {
        let id = request.document().id().as_str().to_owned();
        if self.documents.lock().unwrap().contains_key(&id) {
            return Err(searchkernel_ir::PortableError::new(
                searchkernel_ir::ErrorCode::AlreadyExists,
                format!("document already exists: {id}"),
            ));
        }
        self.store(request.document());
        Ok(InsertResult { id })
    }

    fn upsert(&self, request: UpsertRequestIr) -> searchkernel_ir::Result<UpsertResult> {
        let id = request.document().id().as_str().to_owned();
        self.store(request.document());
        Ok(UpsertResult { id })
    }

    fn patch(&self, request: PatchRequestIr) -> searchkernel_ir::Result<PatchResult> {
        let id = request.id().as_str().to_owned();
        let mut documents = self.documents.lock().unwrap();
        let fields = documents.entry(id.clone()).or_default();
        for operation in request.operations() {
            match operation {
                PatchOperationIr::Set { field, value } => {
                    fields.insert(field.as_str().to_owned(), value.clone());
                }
                PatchOperationIr::Unset { field } => {
                    fields.remove(field.as_str());
                }
            }
        }
        Ok(PatchResult { id })
    }

    fn delete(&self, request: DeleteRequestIr) -> searchkernel_ir::Result<DeleteResult> {
        let id = request.id().as_str().to_owned();
        let deleted = self.documents.lock().unwrap().remove(&id).is_some();
        Ok(DeleteResult { id, deleted })
    }

    fn get(&self, request: GetRequestIr) -> searchkernel_ir::Result<GetResult> {
        let id = request.id().as_str();
        let document = self
            .documents
            .lock()
            .unwrap()
            .get(id)
            .map(|fields| DocumentResult {
                id: id.to_owned(),
                fields: Self::fields_to_result(fields),
            });
        Ok(GetResult { document })
    }

    fn search(&self, _request: SearchRequestIr) -> searchkernel_ir::Result<SearchResult> {
        let documents = self.documents.lock().unwrap();
        let hits = documents
            .iter()
            .map(|(id, fields)| searchkernel_ir::Hit {
                id: id.clone(),
                score: Some(1.0),
                fields: Self::fields_to_result(fields),
                highlights: vec![],
            })
            .collect::<Vec<_>>();
        Ok(SearchResult {
            total_hits: searchkernel_ir::TotalHits {
                value: hits.len() as u64,
                relation: searchkernel_ir::TotalHitsRelation::Exact,
            },
            hits,
            facets: vec![],
        })
    }

    fn analyze(
        &self,
        _request: searchkernel_ir::AnalyzeRequestIr,
    ) -> searchkernel_ir::Result<searchkernel_ir::AnalyzeResult> {
        Err(searchkernel_ir::PortableError::unsupported(
            "test_backend",
            None::<String>,
            "analyze is not part of the C4 facade",
        ))
    }
}

#[test]
fn public_api_round_trips_crud_and_match_all_search() {
    let backend = Arc::new(StatefulBackend::default());
    let index = Index::from_backend(definition(), backend).unwrap();
    let product = Product {
        title: "wireless headphones".into(),
        brand: "Acme".into(),
        price: 449.0,
        in_stock: true,
    };

    index.insert("sku-1", &product).unwrap();
    assert_eq!(
        index.get::<Product>("sku-1").unwrap(),
        Some(product.clone())
    );
    let error = index.insert("sku-1", &product).unwrap_err();
    assert_eq!(
        error.portable().unwrap().code,
        searchkernel_ir::ErrorCode::AlreadyExists
    );

    let clone = index.clone();
    clone
        .patch("sku-1", Patch::new().set("price", 450.0).unset("brand"))
        .unwrap();
    let patched: JsonValue = index.get("sku-1").unwrap().unwrap();
    assert_eq!(patched["price"], 450.0);
    assert!(patched.get("brand").is_none());

    index.upsert("sku-1", &product).unwrap();
    index.upsert("sku-2", &product).unwrap();
    let result = index.search("").execute::<Product>().unwrap();
    assert_eq!(result.total_hits.value, 2);
    assert_eq!(
        result.total_hits.relation,
        searchkernel::TotalHitsRelation::Exact
    );
    assert_eq!(result.hits[0].score, Some(1.0));
    assert_eq!(result.hits[1].document, product);

    assert!(index.delete("sku-1").unwrap());
    assert!(!index.delete("sku-1").unwrap());
    assert_eq!(index.get::<Product>("sku-1").unwrap(), None);
}
