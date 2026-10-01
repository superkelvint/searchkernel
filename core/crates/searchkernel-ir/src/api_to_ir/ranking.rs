use super::*;
use crate::model::*;
use searchkernel_protocol::wire::v1 as api;
use std::collections::BTreeMap;
impl IndexContext {
    pub fn ranking_request_to_ir(
        &self,
        ranking: Option<api::RankingRequest<'_>>,
    ) -> Result<RankingRequestIr> {
        let (ranking, rank_properties) = super::search::ranking_to_input(ranking)?;
        let request = resolve_search(
            self,
            SearchInput {
                query: QuerySourceInput::MatchAll,
                search_fields: None,
                filters: Vec::new(),
                textual_filters: Vec::new(),
                sorts: Vec::new(),
                grouping: Vec::new(),
                ranking,
                query_inputs: BTreeMap::new(),
                rank_properties,
                query_model: QueryModelInput::default(),
                projection: None,
                highlighting: None,
                presentation: PresentationInput::default(),
                properties: BTreeMap::new(),
                query_profile: None,
                search_chain: None,
                timeout: None,
                limit: None,
                offset: None,
            },
        )?;
        Ok(request.ranking_request().clone())
    }
}
