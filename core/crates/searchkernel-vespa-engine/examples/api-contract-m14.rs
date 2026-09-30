//! Compile-only M14 contract: HNSW tensor schema + explicit NN controls.

use searchkernel_vespa_engine::{
    DistanceMetric, Field, FieldType, HnswIndexConfig, NearestNeighborOptions, Query, Schema,
    SearchRequest, TensorValue,
};

fn main() {
    let schema = Schema::new("typea").field(
        Field::new("embedding", FieldType::Tensor("tensor<float>(x[3])".into()))
            .attribute()
            .distance_metric(DistanceMetric::Euclidean)
            .hnsw(
                HnswIndexConfig::new()
                    .max_links_per_node(4)
                    .neighbors_to_explore_at_insert(16),
            ),
    );

    let query = Query::NearestNeighborWithOptions {
        document_field: "embedding".into(),
        query_tensor: "query_embedding".into(),
        options: NearestNeighborOptions {
            target_num_hits: 3,
            approximate: true,
            distance_threshold: Some(100.0),
            explore_additional_hits: 5,
        },
    };
    let request = SearchRequest::new(query)
        .query_tensor(
            "query_embedding",
            TensorValue::dense("tensor<float>(x[3])", vec![1.0, 0.0, 0.0]),
        )
        .limit(3);

    let _ = (schema, request);
}
