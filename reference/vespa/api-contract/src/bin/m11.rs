use searchkernel_vespa_engine::{Document, Engine, Query, SearchRequest};

fn assert_send_sync<T: Send + Sync>() {}

fn main() {
    assert_send_sync::<Engine>();
    let _open = Engine::open("data", "config");
    let _query = Query::from_yql("select * from sources * where title contains \"term\"");
    let _text = Document::new("id").field("title", "text");
    let _term = SearchRequest::new(Query::term("title", "term"));
}
