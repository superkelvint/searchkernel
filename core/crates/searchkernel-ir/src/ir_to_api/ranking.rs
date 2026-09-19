use super::*;

pub(super) fn ranking_request_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    ranking: &ir::RankingRequestIr,
) -> flatbuffers::WIPOffset<api::RankingRequest<'a>> {
    let properties = (!ranking.properties.is_empty()).then(|| {
        let entries = ranking
            .properties
            .iter()
            .map(|(key, value)| {
                let key = builder.create_string(key);
                let value = builder.create_string(value);
                api::StringEntry::create(
                    builder,
                    &api::StringEntryArgs {
                        key: Some(key),
                        value: Some(value),
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&entries)
    });
    let profile = builder.create_string(&ranking.profile);
    api::RankingRequest::create(
        builder,
        &api::RankingRequestArgs {
            profile: Some(profile),
            properties,
        },
    )
}
