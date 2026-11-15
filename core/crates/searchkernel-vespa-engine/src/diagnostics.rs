//! Owned ABI 17 diagnostic projection. Engine trace syntax stays in C++.
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticStatus {
    Available,
    Unsupported,
    UnavailableForQuery,
    Truncated,
    TimedOut,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticSection {
    MatchingProfiler,
    FirstPhaseProfiler,
    SecondPhaseProfiler,
    IteratorTree,
    Blueprint,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticNode {
    /// Preorder parent index; -1 denotes a report/tree root.
    pub parent: i32,
    pub thread: u32,
    pub name: String,
    pub count: Option<u64>,
    pub total_ms: Option<f64>,
    pub self_ms: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticProjection {
    pub engine: String,
    pub revision: String,
    pub native_abi: u32,
    pub nodes: Vec<DiagnosticNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticEvidence {
    pub section: DiagnosticSection,
    pub status: DiagnosticStatus,
    pub projection: Option<DiagnosticProjection>,
}

pub(crate) fn decode_projection(bytes: &[u8]) -> crate::Result<DiagnosticProjection> {
    if bytes.len() > 65_536 {
        return Err(crate::Error::message(
            "native diagnostic projection exceeds bounds",
        ));
    }
    let value: DiagnosticProjection = serde_json::from_slice(bytes)
        .map_err(|_| crate::Error::message("invalid native diagnostic projection"))?;
    if value.nodes.len() > 128
        || value.engine.len() > 64
        || value.revision.len() != 40
        || !value.revision.bytes().all(|b| b.is_ascii_hexdigit())
        || value.native_abi != searchkernel_vespa_bindings::EXPECTED_ABI
    {
        return Err(crate::Error::message(
            "invalid native diagnostic provenance/bounds",
        ));
    }
    for (index, node) in value.nodes.iter().enumerate() {
        if node.count.is_some_and(|v| v > i64::MAX as u64)
            || node.name.len() > 256
            || node.parent < -1
            || node.parent >= index as i32
            || node.thread > 1024
            || [node.total_ms, node.self_ms]
                .into_iter()
                .flatten()
                .any(|v| !v.is_finite() || v < 0.0)
            || (node.parent >= 0 && value.nodes[node.parent as usize].thread != node.thread)
        {
            return Err(crate::Error::message("invalid native diagnostic node"));
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn projection(nodes: serde_json::Value) -> Vec<u8> {
        // This is the Search Kernel ABI schema, independent of engine trace.
        serde_json::to_vec(&serde_json::json!({"engine":"Vespa", "revision": "2300ef0412bdb76f227ae3b497c8ca5e73c5da4e",
            "native_abi":searchkernel_vespa_bindings::EXPECTED_ABI, "nodes":nodes})).unwrap()
    }
    #[test]
    fn owned_nodes_preserve_root_total_self_count_and_leaf_absence() {
        let value = decode_projection(&projection(serde_json::json!([
            {"parent":-1,"thread":0,"name":"profile","total_ms":12.0},
            {"parent":0,"thread":0,"name":"parent","count":2,"total_ms":12.0,"self_ms":5.0},
            {"parent":1,"thread":0,"name":"child","count":3,"total_ms":7.0}
        ])))
        .unwrap();
        assert_eq!(value.nodes[0].total_ms, Some(12.0));
        assert_eq!(value.nodes[1].self_ms, Some(5.0));
        assert_eq!(value.nodes[2].count, Some(3));
        assert_eq!(value.nodes[2].self_ms, None);
    }
    #[test]
    fn decoder_rejects_cycles_invalid_timing_and_oversized_data() {
        for node in [
            serde_json::json!({"parent":0,"thread":0,"name":"cycle"}),
            serde_json::json!({"parent":-1,"thread":0,"name":"bad","total_ms":-1.0}),
            serde_json::json!({"parent":-1,"thread":0,"name":"x".repeat(257)}),
        ] {
            assert!(decode_projection(&projection(serde_json::json!([node]))).is_err());
        }
        assert!(decode_projection(&vec![b' '; 65_537]).is_err());
    }
}
