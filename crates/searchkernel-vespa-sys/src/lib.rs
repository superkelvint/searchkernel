#![allow(non_camel_case_types)]

mod abi;
pub use abi::EXPECTED_ABI;

include!("bindings.rs");
