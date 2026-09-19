use super::*;
use crate::model::*;
use searchkernel_protocol::v2 as api;
use std::collections::BTreeMap;

impl IndexContext {
    pub fn ranking_request_to_ir(
        &self,
        ranking: Option<api::RankingRequest<'_>>,
    ) -> Result<RankingRequestIr> {
        ranking_request_to_ir(ranking)
    }
}

pub(crate) fn ranking_request_to_ir(
    value: Option<api::RankingRequest<'_>>,
) -> Result<RankingRequestIr> {
    let Some(value) = value else {
        return Ok(RankingRequestIr {
            profile: "default".to_owned(),
            properties: BTreeMap::new(),
        });
    };
    let mut properties = BTreeMap::new();
    if let Some(entries) = value.properties() {
        for (index, entry) in entries.iter().enumerate() {
            let key = entry.key().to_owned();
            if properties
                .insert(key.clone(), entry.value().unwrap_or_default().to_owned())
                .is_some()
            {
                return Err(invalid(
                    "duplicate_ranking_property",
                    Some(format!("ranking.properties[{index}]")),
                    format!("ranking property `{key}` occurs more than once"),
                ));
            }
        }
    }
    // An omitted/empty profile is the portable default.  Resolve that here,
    // before any backend lowering, so adapters never have to invent a
    // fallback profile name.
    let profile = value
        .profile()
        .filter(|profile| !profile.is_empty())
        .unwrap_or("default");
    Ok(RankingRequestIr {
        profile: profile.to_owned(),
        properties,
    })
}
