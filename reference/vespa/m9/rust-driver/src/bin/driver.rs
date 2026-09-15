use searchkernel_m9_verifier::{compile_schema,load,populate,run_positive};
use searchkernel_vespa_engine::Engine;
use std::path::PathBuf;
fn main(){
 let mut a=std::env::args_os().skip(1);
 let scenario=PathBuf::from(a.next().expect("scenario"));
 let config=PathBuf::from(a.next().expect("config"));
 let data=PathBuf::from(a.next().expect("data"));
 assert!(a.next().is_none());
 let root=load(&scenario); compile_schema(&root,&config); std::fs::create_dir_all(&data).unwrap();
 let engine=Engine::open(&data,&config).unwrap(); populate(&engine,&root);
 println!("{}",serde_json::to_string(&run_positive(&engine,&root)).unwrap());
 engine.close().unwrap();
}
