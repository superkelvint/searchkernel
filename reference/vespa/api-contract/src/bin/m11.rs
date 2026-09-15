use searchkernel_vespa_engine::{Engine, Query, TextDocument, TextQuery};

fn assert_send_sync<T: Send + Sync>() {}

fn main() {
    assert_send_sync::<Engine>();
    let _open = Engine::open("data", "config");
    let _query = Query::from_yql("select * from sources * where title contains \"term\"");
    let _text = TextDocument {
        id: "id",
        field: "title",
        text: "text",
    };
    let _term = TextQuery::Term {
        field: "title",
        text: "term",
    };
}
