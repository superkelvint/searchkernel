use super::*;

pub(crate) struct NativeError(pub(crate) [u8; sys::SK_ERROR_MESSAGE_CAPACITY as usize]);

impl NativeError {
    pub(crate) fn new() -> Self {
        Self([0; 512])
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }

    pub(crate) fn capacity(&self) -> u32 {
        self.0.len() as u32
    }
}

pub(crate) struct NativeDocument(pub(crate) NonNull<sys::sk_document>);
impl Drop for NativeDocument {
    fn drop(&mut self) {
        unsafe { sys::sk_document_destroy(self.0.as_ptr()) };
    }
}

pub(crate) struct NativeUpdate(pub(crate) NonNull<sys::sk_document_update>);
impl Drop for NativeUpdate {
    fn drop(&mut self) {
        unsafe { sys::sk_document_update_destroy(self.0.as_ptr()) };
    }
}

pub(crate) struct NativeAttributeDocument(pub(crate) NonNull<sys::sk_attribute_document>);
impl Drop for NativeAttributeDocument {
    fn drop(&mut self) {
        unsafe { sys::sk_attribute_document_destroy(self.0.as_ptr()) };
    }
}

pub(crate) struct NativeAttributeUpdate(pub(crate) NonNull<sys::sk_attribute_update>);
impl Drop for NativeAttributeUpdate {
    fn drop(&mut self) {
        unsafe { sys::sk_attribute_update_destroy(self.0.as_ptr()) };
    }
}

pub(crate) fn require_ptr<T>(pointer: *mut T, name: &str) -> Result<NonNull<T>, Error> {
    NonNull::new(pointer)
        .ok_or_else(|| Error::corrupt(format!("native operation returned a null {name}")))
}
