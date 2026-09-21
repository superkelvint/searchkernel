use searchkernel_m14_verifier::{load,run_low_level};
use std::path::PathBuf;
fn main(){
    let mut a=std::env::args_os().skip(1);
    let scenario=PathBuf::from(a.next().expect("scenario"));
    let config=PathBuf::from(a.next().expect("config"));
    let data=PathBuf::from(a.next().expect("data"));
    assert!(a.next().is_none());
    let root=load(&scenario);
    println!("{}",serde_json::to_string(&run_low_level(&root,&config,&data).expect("M14 low-level run")).unwrap());
}
