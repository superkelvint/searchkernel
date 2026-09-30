#[path = "support/m12.rs"]
mod m12;

use crate::m12::{load, prepare_fixture};
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let scenario = PathBuf::from(args.next().expect("scenario"));
    let config = PathBuf::from(args.next().expect("config"));
    let data = PathBuf::from(args.next().expect("data"));
    assert!(args.next().is_none());

    let root = load(&scenario);
    prepare_fixture(&root, &config, &data).expect("prepare M12 fixture through SearchKernel");
}
