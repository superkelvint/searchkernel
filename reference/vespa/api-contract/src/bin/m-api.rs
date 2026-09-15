//! Compile-only contract for the stable, Vespa-facing Rust API.

use searchkernel_vespa_engine::{
    Document, DocumentUpdate, Engine, Query, SearchRequest,
};
use std::path::Path;

#[allow(dead_code)]
fn m_api_surface(data: &Path, config: &Path) -> searchkernel_vespa_engine::Result<()> {
    let engine = Engine::open(data, config)?;

    engine.put(
        Document::new("sku-42")
            .field("title", "Wireless Headphones")
            .field("price", 149_i64)
            .field("tags", ["audio", "wireless"]),
    )?;

    engine.update(
        DocumentUpdate::new("sku-42")
            .assign("price", 129_i64)
            .clear("sale_price"),
    )?;

    let _result = engine.search(
        SearchRequest::new(Query::term("title", "wireless"))
            .rank_profile("default")
            .limit(20),
    )?;
    let _document = engine.get("sku-42")?;
    let _deleted = engine.delete("sku-42")?;
    Ok(())
}

fn main() {}
