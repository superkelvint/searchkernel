//! The single FlatBuffers encoder retained by the Rust facade.
//!
//! This is used only to preserve the caller's schema intent in the manifest.
//! Runtime request execution and runtime schema resolution do not use it.

use crate::schema::IndexDefinition;

/// Encode the caller-visible unresolved schema intent through the shared
/// canonical FlatBuffers encoder.
///
/// This path is deliberately fallible: if the public definition cannot be
/// represented faithfully, callers must surface that error rather than falling
/// back to a narrower handwritten representation.
pub(crate) fn try_encode_portable_definition(
    definition: &IndexDefinition,
) -> crate::error::Result<Vec<u8>> {
    let input = crate::schema::index_definition_to_input(definition)?;
    Ok(searchkernel_ir::wire::v1::index_definition_input_to_api(
        &input,
    ))
}

#[cfg(test)]
pub(crate) fn encode_portable_definition(definition: &IndexDefinition) -> Vec<u8> {
    try_encode_portable_definition(definition)
        .expect("test schema should encode through the shared portable definition path")
}
