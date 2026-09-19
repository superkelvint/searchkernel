use super::*;
use crate::model::*;
use searchkernel_protocol::v2 as api;
use std::collections::BTreeMap;

impl IndexContext {
    pub fn ranking_to_ir(
        &self,
        ranking: Option<api::RankingRequest<'_>>,
    ) -> Result<RankingPolicyIr> {
        ranking_to_ir(ranking)
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
    Ok(RankingRequestIr {
        profile: value.profile().unwrap_or_default().to_owned(),
        properties,
    })
}
pub(crate) fn ranking_to_ir(r: Option<api::RankingRequest<'_>>) -> Result<RankingPolicyIr> {
    let Some(r) = r else {
        return Ok(RankingPolicyIr::DefaultRelevanceV1);
    };
    let profile = r.profile().ok_or_else(|| {
        invalid(
            "invalid_ranking",
            Some("ranking"),
            "ranking request must select a profile",
        )
    })?;
    match profile {
        "" | "default" | "default-relevance" => Ok(RankingPolicyIr::DefaultRelevanceV1),
        "bm25" => Ok(RankingPolicyIr::Bm25),
        // The concrete profile remains losslessly represented in
        // `RankingRequestIr`; this compatibility classification is used only
        // by the older native lowering path.
        _ => Ok(RankingPolicyIr::DefaultRelevanceV1),
    }
}
