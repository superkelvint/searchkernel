//! Wire-major routing. Version-specific FlatBuffers types stay below this module.

use super::{DispatchError, Dispatcher};

pub(super) mod v1;

pub(super) fn dispatch(dispatcher: &Dispatcher, request: &[u8]) -> Result<Vec<u8>, DispatchError> {
    match v1::protocol_version(request)? {
        1 => v1::dispatch(dispatcher, request),
        version => Err(DispatchError::UnsupportedProtocol(version)),
    }
}
