use searchkernel_dispatch::Dispatcher;
use searchkernel_ir::{Backend,BackendSession,IndexContext,index_definition_to_ir};
use searchkernel_ir_adapter::VespaBackend;
use searchkernel_m14_verifier::load;
use searchkernel_protocol::json::{request_json_to_flatbuffer,response_flatbuffer_to_json};
use serde_json::{Value as Json,json};
use std::{fs,path::PathBuf,sync::Arc};

fn definition_ir(path:&PathBuf)->searchkernel_ir::IndexDefinitionIr{
    let bytes=fs::read(path).expect("read M14 definition");
    let root=searchkernel_ir::__flatbuffers::root::<searchkernel_ir::api::IndexDefinition>(&bytes).expect("parse M14 definition");
    index_definition_to_ir(root).expect("definition to IR")
}
fn open(definition:&PathBuf,config:&PathBuf,data:&PathBuf)->Dispatcher{
    let ir=definition_ir(definition);
    if !config.join("searchkernel-schema.json").exists(){
        searchkernel_ir_adapter::compile_index_definition(&ir,config).expect("compile M14 HNSW definition");
    }
    let context=Arc::new(IndexContext::from_definition_ir(ir));
    let backend:Arc<dyn Backend>=Arc::new(VespaBackend::open(data,config).expect("open M14 backend"));
    Dispatcher::new(context,BackendSession::from_backend(backend))
}
fn envelope(kind:&str,payload:Json)->Vec<u8>{serde_json::to_vec(&json!({"protocol_version":2,"payload_type":kind,"payload":payload})).unwrap()}
fn dispatch(d:&Dispatcher,kind:&str,payload:Json)->Json{
    let req=request_json_to_flatbuffer(&envelope(kind,payload)).expect("request FB");
    let resp=d.dispatch(&req).expect("dispatch M14 request");
    serde_json::from_slice(&response_flatbuffer_to_json(&resp).expect("response JSON")).unwrap()
}
fn dense(root:&Json,values:&Json)->Json{json!({"kind_type":"TensorValue","kind":{"type_spec":root["tensor_field"]["type_spec"],"payload_type":"DenseTensorValues","payload":{"values":values}}})}
fn doc(root:&Json,item:&Json)->Json{json!({"id":item["id"],"fields":[{"key":root["tensor_field"]["name"],"value":dense(root,&item["values"])}]})}
fn annotations(step:&Json)->Vec<Json>{
    let mut out=vec![
        json!({"key":"totalTargetHits","value":{"kind_type":"UInt64Value","kind":{"value":step["target_hits"].as_u64().unwrap().to_string()}}}),
        json!({"key":"approximate","value":{"kind_type":"BoolValue","kind":{"value":step["approximate"]}}}),
        json!({"key":"exploreAdditionalHits","value":{"kind_type":"UInt64Value","kind":{"value":step["explore_additional_hits"].as_u64().unwrap().to_string()}}}),
    ];
    if !step["distance_threshold"].is_null(){out.push(json!({"key":"distanceThreshold","value":{"kind_type":"Float64Value","kind":{"value":step["distance_threshold"]}}}));}
    out
}
fn hits(response:&Json)->Vec<String>{
    fn visit(node:&Json,out:&mut Vec<String>){
        match node.get("node_type").and_then(Json::as_str){
            Some("Hit")|Some("ResultHit")=>if let Some(id)=node.get("node").and_then(|n|n.get("id")).and_then(Json::as_str){out.push(id.to_owned())},
            Some("ResultGroup")=>if let Some(children)=node.get("node").and_then(|n|n.get("children")).and_then(Json::as_array){for child in children{visit(child,out)}},
            _=>{}
        }
    }
    let mut out=Vec::new(); if let Some(root)=response.get("payload").and_then(|p|p.get("root")){visit(root,&mut out)} out
}
fn query(d:&Dispatcher,root:&Json,step:&Json)->Vec<String>{
    let response=dispatch(d,"SearchKernel_V2_SearchRequest",json!({
        "query":{"source_type":"TypedQuery","source":{"query":{"expr_type":"QueryNearestNeighbor","expr":{"document_field":{"path":root["tensor_field"]["name"]},"query_tensor":"query_embedding"},"annotations":annotations(step)}}},
        "ranking":{"profile":"semantic"},
        "inputs":[{"name":"query_embedding","input_type":"LiteralQueryInput","input":{"value":dense(root,&root["query"])}}],
        "presentation":{"fields":{"fields":[]}},
        "limit":{"value":step["limit"]}
    }));
    assert_eq!(response["payload_type"],"SearchKernel_V2_SearchResult"); hits(&response)
}
fn feed(d:&Dispatcher,root:&Json){for item in root["documents"].as_array().unwrap(){let response=dispatch(d,"SearchKernel_V2_InsertRequest",json!({"document":doc(root,item)}));assert_eq!(response["payload_type"],"SearchKernel_V2_InsertResult");}}
fn patch(d:&Dispatcher,root:&Json,id:&str,values:&Json){let response=dispatch(d,"SearchKernel_V2_PatchRequest",json!({"id":id,"operations":[{"operation_type":"SetField","operation":{"field":root["tensor_field"]["name"],"value":dense(root,values)}}]}));assert_eq!(response["payload_type"],"SearchKernel_V2_PatchResult");}
fn run(root:&Json,definition:&PathBuf,config:&PathBuf,data:&PathBuf)->Json{
    if root["id"]=="hnsw-query-controls"{
        let d=open(definition,config,data); feed(&d,root); let mut steps=Vec::new(); for step in root["steps"].as_array().unwrap(){steps.push(json!({"id":step["id"],"hits":query(&d,root,step)}));} d.close().unwrap(); return json!({"id":root["id"],"steps":steps});
    }
    let opts=&root["query_options"]; let update_id=root["mutation"]["update_id"].as_str().unwrap(); let delete_id=root["mutation"]["delete_id"].as_str().unwrap();
    let d=open(definition,config,data); feed(&d,root); let before_update=query(&d,root,opts); patch(&d,root,update_id,&root["mutation"]["update_values"]); let after_update=query(&d,root,opts); d.close().unwrap();
    let d=open(definition,config,data); let after_reopen=query(&d,root,opts); let deleted=dispatch(&d,"SearchKernel_V2_DeleteRequest",json!({"id":delete_id})); assert_eq!(deleted["payload"]["deleted"],true); let after_delete=query(&d,root,opts); d.close().unwrap();
    let d=open(definition,config,data); let after_delete_reopen=query(&d,root,opts); d.close().unwrap();
    json!({"id":root["id"],"before_update":before_update,"after_update":after_update,"after_reopen":after_reopen,"after_delete":after_delete,"after_delete_reopen":after_delete_reopen})
}
fn main(){let mut a=std::env::args_os().skip(1);let scenario=PathBuf::from(a.next().expect("scenario"));let definition=PathBuf::from(a.next().expect("definition"));let config=PathBuf::from(a.next().expect("config"));let data=PathBuf::from(a.next().expect("data"));assert!(a.next().is_none());let root=load(&scenario);println!("{}",serde_json::to_string(&run(&root,&definition,&config,&data)).unwrap());}
