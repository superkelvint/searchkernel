use super::*;

/// Encode a portable error as a standalone FlatBuffers `Error` table.
pub fn error_to_api(error: &PortableError) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = error_offset(&mut builder, error);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

/// Encode a portable error into an existing FlatBuffers builder.
pub fn error_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    error: &PortableError,
) -> flatbuffers::WIPOffset<api::Error<'a>> {
    let message = builder.create_string(&error.message);
    let mut detail_offsets = Vec::with_capacity(error.details.len());
    for detail in &error.details {
        let code = builder.create_string(&detail.code);
        let path = detail.path.as_deref().map(|v| builder.create_string(v));
        let text = builder.create_string(&detail.message);
        let metadata = if detail.metadata.is_empty() {
            None
        } else {
            let mut entries = Vec::with_capacity(detail.metadata.len());
            for (key, value) in &detail.metadata {
                let key = builder.create_string(key);
                let value = builder.create_string(value);
                entries.push(api::StringEntry::create(
                    builder,
                    &api::StringEntryArgs {
                        key: Some(key),
                        value: Some(value),
                    },
                ));
            }
            Some(builder.create_vector(&entries))
        };
        detail_offsets.push(api::ErrorDetail::create(
            builder,
            &api::ErrorDetailArgs {
                code: Some(code),
                path,
                message: Some(text),
                metadata,
            },
        ));
    }
    let details = (!detail_offsets.is_empty()).then(|| builder.create_vector(&detail_offsets));
    api::Error::create(
        builder,
        &api::ErrorArgs {
            code: error_code(error.code),
            message: Some(message),
            details,
            retryable: error.retryable,
        },
    )
}

pub(super) fn error_code(code: ErrorCode) -> api::ErrorCode {
    match code {
        ErrorCode::InvalidArgument => api::ErrorCode::InvalidArgument,
        ErrorCode::NotFound => api::ErrorCode::NotFound,
        ErrorCode::AlreadyExists => api::ErrorCode::AlreadyExists,
        ErrorCode::FailedPrecondition => api::ErrorCode::FailedPrecondition,
        ErrorCode::Conflict => api::ErrorCode::Conflict,
        ErrorCode::Unsupported => api::ErrorCode::Unsupported,
        ErrorCode::ResourceExhausted => api::ErrorCode::ResourceExhausted,
        ErrorCode::Internal => api::ErrorCode::Internal,
        ErrorCode::Unavailable => api::ErrorCode::Unavailable,
        ErrorCode::DeadlineExceeded => api::ErrorCode::DeadlineExceeded,
    }
}
