#[path = "support/m12.rs"]
mod m12;

use crate::m12::{load, run_low_level};
use searchkernel_vespa_engine::Engine;
use std::path::PathBuf;
fn main() {
    let mut a = std::env::args_os().skip(1);
    let scenario = PathBuf::from(a.next().expect("scenario"));
    let config = PathBuf::from(a.next().expect("config"));
    let data = PathBuf::from(a.next().expect("data"));
    assert!(a.next().is_none());
    let root = load(&scenario);
    let engine = Engine::open(&data, &config).expect("open prepared M12 fixture");
    println!(
        "{}",
        serde_json::to_string(&run_low_level(&engine, &root).expect("M12 low-level search"))
            .unwrap()
    );
    engine.close().expect("close M12 fixture");
}
