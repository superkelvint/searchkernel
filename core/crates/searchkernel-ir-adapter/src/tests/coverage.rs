use super::*;

#[test]
fn vespa_coverage_maps_to_portable_coverage_without_issue_fabrication() {
    let full = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 10,
        covered: 10,
        target_active: 10,
        degradation_flags: 0,
    });
    assert_eq!(full.coverage_percent, Some(100.0));
    assert_eq!(full.full, Some(true));
    assert_eq!(full.results_full, Some(1));

    let partial = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 10,
        covered: 4,
        target_active: 20,
        degradation_flags: 0,
    });
    assert_eq!(partial.coverage_percent, Some(40.0));
    assert_eq!(partial.documents, Some(4));
    assert_eq!(partial.full, Some(false));
    assert_eq!(partial.results_full, Some(0));

    let match_phase = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 10,
        covered: 10,
        target_active: 20,
        degradation_flags: 0x01,
    });
    assert_eq!(match_phase.coverage_percent, Some(100.0));
    assert_eq!(match_phase.full, Some(false));
    assert_eq!(match_phase.results_full, Some(0));
    assert_eq!(
        match_phase.degraded.as_ref().unwrap().match_phase,
        Some(true)
    );

    let timeout = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 10,
        covered: 10,
        target_active: 20,
        degradation_flags: 0x02,
    });
    assert_eq!(timeout.coverage_percent, Some(100.0));
    assert_eq!(timeout.full, Some(false));
    assert_eq!(timeout.results_full, Some(0));
    assert_eq!(timeout.degraded.as_ref().unwrap().timeout, Some(true));

    let ann_timeout = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 10,
        covered: 10,
        target_active: 20,
        degradation_flags: 0x04,
    });
    assert_eq!(ann_timeout.coverage_percent, Some(100.0));
    assert_eq!(ann_timeout.full, Some(false));
    assert_eq!(ann_timeout.results_full, Some(0));
    assert_eq!(
        ann_timeout.degraded.as_ref().unwrap().ann_timeout,
        Some(true)
    );

    let empty = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 0,
        covered: 0,
        target_active: 10,
        degradation_flags: 0,
    });
    assert_eq!(empty.coverage_percent, Some(100.0));
    assert_eq!(empty.full, Some(true));

    let empty_timeout = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 0,
        covered: 0,
        target_active: 0,
        degradation_flags: 0x02,
    });
    assert_eq!(empty_timeout.coverage_percent, Some(0.0));
    assert_eq!(empty_timeout.full, Some(false));

    let non_ideal_corpus = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 8,
        covered: 8,
        target_active: 10,
        degradation_flags: 0,
    });
    assert_eq!(non_ideal_corpus.coverage_percent, Some(100.0));
    assert_eq!(non_ideal_corpus.full, Some(true));
    assert_eq!(
        non_ideal_corpus.degraded.as_ref().unwrap().non_ideal_state,
        None
    );

    assert_eq!(full.documents, Some(10));
    assert_eq!(full.nodes, Some(1));
    assert_eq!(full.results, Some(1));
    assert!(portable_errors_from_native_issues(&["advisory".to_owned()]).is_empty());
}
