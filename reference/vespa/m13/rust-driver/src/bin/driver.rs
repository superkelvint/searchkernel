use searchkernel_m13_verifier::{load, run_low_level};
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let scenario = PathBuf::from(args.next().expect("scenario"));
    let config = PathBuf::from(args.next().expect("config"));
    let data = PathBuf::from(args.next().expect("data"));
    assert!(args.next().is_none());
    let root = load(&scenario);
    println!("{}", serde_json::to_string(&run_low_level(&root, &config, &data)).unwrap());
}
