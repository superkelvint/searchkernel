//! Compile-only M13 contract: dense tensor document CRUD through generic APIs.

use searchkernel_vespa_engine::{Document, DocumentUpdate, TensorValue};

fn main() {
    let tensor = TensorValue::dense("tensor<float>(x[2])", vec![1.0, 0.0]);
    let document = Document::new("id:ns:typea::one").field("embedding", tensor.clone());
    let update = DocumentUpdate::new("id:ns:typea::one")
        .assign("embedding", tensor)
        .clear("other_field");
    let _ = (document, update);
}
