//! Compile-only M12 contract: dense query tensors + nearest-neighbor query.

use searchkernel_vespa_engine::{Query, SearchRequest, TensorValue};

fn main() {
    let q = TensorValue::dense("tensor<float>(x[3])", vec![1.0, 0.0, 0.0]);
    let request = SearchRequest::new(Query::nearest_neighbor("embedding", "q_embedding"))
        .query_tensor("q_embedding", q)
        .query_feature("boost", 2.0)
        .limit(3);
    let _ = request;
}
