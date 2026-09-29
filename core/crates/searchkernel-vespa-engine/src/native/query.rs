use super::*;

pub(crate) struct NativeQuery(pub(crate) NonNull<sys::sk_query>);
impl Drop for NativeQuery {
    fn drop(&mut self) {
        unsafe { sys::sk_query_destroy(self.0.as_ptr()) };
    }
}

pub(crate) struct NativeNearestNeighborOptions(
    pub(crate) NonNull<sys::sk_nearest_neighbor_options>,
);
impl Drop for NativeNearestNeighborOptions {
    fn drop(&mut self) {
        unsafe { sys::sk_nearest_neighbor_options_destroy(self.0.as_ptr()) };
    }
}

pub(crate) fn nearest_neighbor_options(
    options: &crate::query::NearestNeighborOptions,
) -> Result<NativeNearestNeighborOptions, Error> {
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_nearest_neighbor_options_create(
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    let options_handle =
        NativeNearestNeighborOptions(require_ptr(pointer, "nearest-neighbor options")?);
    if let Some(distance) = options.distance_threshold {
        let mut error = NativeError::new();
        check(
            unsafe {
                sys::sk_nearest_neighbor_options_set_distance_threshold(
                    options_handle.0.as_ptr(),
                    distance,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            },
            &error,
        )?;
    }
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_nearest_neighbor_options_set_explore_additional_hits(
                options_handle.0.as_ptr(),
                options.explore_additional_hits,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    Ok(options_handle)
}

pub(crate) struct NativeGenericSearchRequest(pub(crate) NonNull<sys::sk_generic_search_request>);
impl Drop for NativeGenericSearchRequest {
    fn drop(&mut self) {
        unsafe { sys::sk_generic_search_request_destroy(self.0.as_ptr()) };
    }
}
