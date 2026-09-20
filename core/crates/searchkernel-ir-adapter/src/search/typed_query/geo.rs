//! Geo query lowering.

use super::typed_query_field;
use searchkernel_ir::{QueryFieldRefIr, Result};
use searchkernel_vespa_engine::Query;

pub(super) fn lower_geo_location(
    field: &QueryFieldRefIr,
    latitude: f64,
    longitude: f64,
    radius: &str,
    path: &str,
) -> Result<Query> {
    Ok(Query::GeoLocation {
        field: typed_query_field(Some(field), &format!("{path}.geo_location.field"))?.to_owned(),
        latitude,
        longitude,
        radius: radius.to_owned(),
    })
}

pub(super) fn lower_geo_bounding_box(
    field: &QueryFieldRefIr,
    south: f64,
    west: f64,
    north: f64,
    east: f64,
    path: &str,
) -> Result<Query> {
    Ok(Query::GeoBoundingBox {
        field: typed_query_field(Some(field), &format!("{path}.geo_bounding_box.field"))?
            .to_owned(),
        south,
        west,
        north,
        east,
    })
}
