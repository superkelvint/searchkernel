//! Nearest-neighbor query lowering.

use super::annotations::{
    annotation_bool, annotation_f64, annotation_nonnegative_u32, annotation_u32,
    reject_unknown_annotations,
};
use super::typed_query_field;
use super::DEFAULT_WEAK_AND_TARGET_HITS;
use searchkernel_ir::{PortableError, QueryFieldRefIr, RequestValueIr, Result};
use searchkernel_vespa_engine::{NearestNeighborOptions, Query};

pub(super) fn lower_nearest_neighbor_options(
    annotations: &std::collections::BTreeMap<String, RequestValueIr>,
    path: &str,
) -> Result<NearestNeighborOptions> {
    reject_unknown_annotations(
        annotations,
        &[
            "targetNumHits",
            "totalTargetHits",
            "approximate",
            "distanceThreshold",
            "exploreAdditionalHits",
        ],
        path,
    )?;
    let target_num_hits = match (
        annotation_u32(annotations, "targetNumHits", path)?,
        annotation_u32(annotations, "totalTargetHits", path)?,
    ) {
        (Some(_), Some(_)) => {
            return Err(PortableError::invalid(
                "invalid_typed_query_annotation",
                Some(path),
                "nearest-neighbor may specify only one of targetNumHits and totalTargetHits",
            ));
        }
        (Some(value), None) | (None, Some(value)) => value,
        (None, None) => DEFAULT_WEAK_AND_TARGET_HITS,
    };
    Ok(NearestNeighborOptions {
        target_num_hits,
        approximate: annotation_bool(annotations, "approximate", path)?.unwrap_or(true),
        distance_threshold: annotation_f64(annotations, "distanceThreshold", path)?,
        explore_additional_hits: annotation_nonnegative_u32(
            annotations,
            "exploreAdditionalHits",
            path,
        )?
        .unwrap_or(0),
    })
}

pub(super) fn lower_nearest_neighbor(
    document_field: &QueryFieldRefIr,
    query_tensor: &str,
    annotations: &std::collections::BTreeMap<String, RequestValueIr>,
    path: &str,
) -> Result<Query> {
    let document_field = typed_query_field(
        Some(document_field),
        &format!("{path}.nearest_neighbor.document_field"),
    )?
    .to_owned();
    let options = lower_nearest_neighbor_options(annotations, path)?;
    if options == NearestNeighborOptions::default() {
        Ok(Query::NearestNeighbor {
            document_field,
            query_tensor: query_tensor.to_owned(),
            target_num_hits: options.target_num_hits,
            approximate: options.approximate,
        })
    } else {
        Ok(Query::NearestNeighborWithOptions {
            document_field,
            query_tensor: query_tensor.to_owned(),
            options,
        })
    }
}
