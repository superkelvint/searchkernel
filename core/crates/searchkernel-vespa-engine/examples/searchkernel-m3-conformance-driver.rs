use searchkernel_vespa_engine::{Language, Linguistics, LinguisticsOptions, StemMode};
use std::io::{self, BufRead};

fn from_hex(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "odd hex length");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("invalid hex"))
        .collect()
}

fn json(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if c < '\u{20}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn options(stemming: &str, remove_accents: &str) -> LinguisticsOptions {
    LinguisticsOptions {
        language: Language::English,
        stem_mode: match stemming {
            "none" => StemMode::None,
            "default" => StemMode::Default,
            _ => panic!("unsupported stemming mode: {stemming}"),
        },
        remove_accents: match remove_accents {
            "0" => false,
            "1" => true,
            _ => panic!("remove_accents must be 0 or 1"),
        },
    }
}

fn main() {
    let mode = std::env::args()
        .nth(1)
        .expect("usage: driver document|query");
    assert!(mode == "document" || mode == "query", "invalid mode");
    let linguistics = Linguistics;

    for line in io::stdin().lock().lines() {
        let line = line.expect("stdin read failed");
        if line.is_empty() {
            continue;
        }
        let parts: Vec<_> = line.split('\t').collect();
        assert_eq!(parts.len(), 4, "invalid protocol line");
        let id = parts[0];
        let opts = options(parts[1], parts[2]);
        let input = String::from_utf8(from_hex(parts[3])).expect("scenario input is not UTF-8");

        if mode == "document" {
            let analysis = linguistics.analyze_document(&input, opts);
            let mut out = format!("{{\"id\":{},\"terms\":[", json(id));
            for (i, term) in analysis.terms.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&format!(
                    "{{\"original\":{},\"term\":{},\"byte_start\":{},\"byte_len\":{},\"position\":{}}}",
                    json(&term.original),
                    json(&term.term),
                    term.byte_start,
                    term.byte_len,
                    term.position
                ));
            }
            out.push_str("]}");
            println!("{out}");
        } else {
            let analysis = linguistics.analyze_query(&input, opts);
            let mut out = format!("{{\"id\":{},\"terms\":[", json(id));
            for (i, term) in analysis.terms.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str("{\"stems\":[");
                for (j, stem) in term.stems.iter().enumerate() {
                    if j > 0 {
                        out.push(',');
                    }
                    out.push_str(&json(stem));
                }
                out.push_str(&format!("],\"position\":{}}}", term.position));
            }
            out.push_str("]}");
            println!("{out}");
        }
    }
}
