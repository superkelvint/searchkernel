use searchkernel_client::model::{AnalyzeRequest,DeleteRequest,DescribeRequest,GetRequest,InsertRequest,InspectQueryRequest,PatchRequest,SearchRequest,UpsertRequest};
use searchkernel_client::{Client,ClientError,Transport};
use std::path::Path; use std::sync::{Arc,Mutex};
#[allow(warnings)] #[path="../../../../crates/searchkernel-ffi/generated/mod.rs"] mod ffi_generated;
use ffi_generated::search_kernel::ffi::v1::{RequestEnvelope,RequestPayload};
#[derive(Debug,Clone)] struct TestError;
#[derive(Default)] struct State{response:Vec<u8>,requests:Vec<Vec<u8>>}
#[derive(Clone)] struct ScriptedTransport(Arc<Mutex<State>>);
impl ScriptedTransport{fn new(response:Vec<u8>)->Self{Self(Arc::new(Mutex::new(State{response,requests:vec![]})))} fn take_single_request(&self)->Vec<u8>{let mut s=self.0.lock().unwrap();assert_eq!(s.requests.len(),1,"exactly one Transport::call");s.requests.pop().unwrap()}}
impl Transport for ScriptedTransport{type Error=TestError;fn call(&self,request:&[u8])->Result<Vec<u8>,Self::Error>{let mut s=self.0.lock().unwrap();s.requests.push(request.to_vec());Ok(s.response.clone())}}
fn fixture(name:&str)->Vec<u8>{let root=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../crates/searchkernel-json/tests/fixtures/responses");std::fs::read(root.join(name)).unwrap()}
fn request_kind(bytes:&[u8])->RequestPayload{assert!(bytes.len()>=8&&&bytes[4..8]==b"SKRQ");flatbuffers::root::<RequestEnvelope<'_>>(bytes).expect("valid SKRQ").payload_type()}
macro_rules! success_case{($method:ident,$request:ty,$response:literal,$kind:ident)=>{{let t=ScriptedTransport::new(fixture($response));let c=Client::new(t.clone());let r=c.$method(<$request>::default());if r.is_err(){panic!("{} failed to decode matching SKRS",stringify!($method));}let b=t.take_single_request();assert_eq!(request_kind(&b),RequestPayload::$kind,"wrong SKRQ for {}",stringify!($method));}}}
fn main(){
 success_case!(insert,InsertRequest,"insert.skrs",SearchKernel_V1_InsertRequest); success_case!(upsert,UpsertRequest,"upsert.skrs",SearchKernel_V1_UpsertRequest); success_case!(patch,PatchRequest,"patch.skrs",SearchKernel_V1_PatchRequest); success_case!(delete,DeleteRequest,"delete.skrs",SearchKernel_V1_DeleteRequest); success_case!(get,GetRequest,"get.skrs",SearchKernel_V1_GetRequest); success_case!(search,SearchRequest,"search.skrs",SearchKernel_V1_SearchRequest); success_case!(analyze,AnalyzeRequest,"analyze.skrs",SearchKernel_V1_AnalyzeRequest); success_case!(describe,DescribeRequest,"describe.skrs",SearchKernel_V1_DescribeRequest); success_case!(inspect_query,InspectQueryRequest,"inspect_query.skrs",SearchKernel_V1_InspectQueryRequest);
 let t=ScriptedTransport::new(fixture("error-00.skrs")); let c=Client::new(t.clone()); let e=match c.search(SearchRequest::default()){Ok(_)=>panic!("portable error must fail"),Err(e)=>e}; match e{ClientError::Portable(_)=>{},ClientError::Transport(_)=>panic!("portable error became transport error"),ClientError::Protocol(m)=>panic!("portable error became protocol error: {m}")}; assert_eq!(request_kind(&t.take_single_request()),RequestPayload::SearchKernel_V1_SearchRequest);

 // Mismatched normal response is a protocol error.
 let t=ScriptedTransport::new(fixture("insert.skrs")); let c=Client::new(t.clone()); let e=match c.search(SearchRequest::default()){Ok(_)=>panic!("mismatched response must fail"),Err(e)=>e}; match e{ClientError::Protocol(_)=>{},ClientError::Portable(_)=>panic!("mismatched normal response became portable error"),ClientError::Transport(_)=>panic!("mismatched normal response became transport error")}; let _=t.take_single_request();
 // Transport failure stays a transport failure.
 struct FailingTransport; impl Transport for FailingTransport{type Error=TestError;fn call(&self,_:&[u8])->Result<Vec<u8>,Self::Error>{Err(TestError)}} let c=Client::new(FailingTransport); let e=match c.search(SearchRequest::default()){Ok(_)=>panic!("transport failure must fail"),Err(e)=>e}; match e{ClientError::Transport(_)=>{},ClientError::Portable(_)=>panic!("transport failure became portable error"),ClientError::Protocol(_)=>panic!("transport failure became protocol error")};
 println!("C6 generated client/Transport behavioral contract PASS");
}
