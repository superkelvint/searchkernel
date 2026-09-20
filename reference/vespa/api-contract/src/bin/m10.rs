//! Compile-only M10 contract: low-level Vespa ranking integration.

use searchkernel_vespa_engine::{
    Field, FieldType, MatchFeatureValue, Query, RankProfile, Schema, SearchRequest,
};

#[allow(dead_code)]
fn m10_surface() {
    let schema = Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed().summary())
        .field(Field::new("body", FieldType::Text).indexed().summary())
        .field(Field::new("popularity", FieldType::Integer).attribute().summary())
        .rank_profile(
            RankProfile::new("product")
                .first_phase("bm25(title) + query(boost) * attribute(popularity)")
                .match_feature("bm25(title)")
                .match_feature("query(boost)")
                .match_feature("attribute(popularity)"),
        );

    let request = SearchRequest::new(Query::term("title", "fox"))
        .rank_profile("product")
        .query_feature("boost", 2.0)
        .rank_property("vespa.matching.numthreadspersearch", "1")
        .limit(10);

    let _ = schema;
    let _ = request;
}

#[allow(dead_code)]
fn result_surface(result: searchkernel_vespa_engine::SearchResult) {
    for hit in result.hits {
        let _score: f64 = hit.score;
        let _features: std::collections::BTreeMap<String, MatchFeatureValue> = hit.match_features;
    }
}

fn main() {}
