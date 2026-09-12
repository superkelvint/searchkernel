//! M4 acceptance driver.
//!
//! Protocol is intentionally tiny. This driver receives *raw* UTF-8 text/query
//! strings. The searchkernel-vespa M4 implementation is responsible for invoking
//! M3 linguistics before crossing the native boundary.

use searchkernel_vespa::{Engine, TextDocument, TextQuery};
use std::io::{self, BufRead, Write};

fn from_hex(hex: &str) -> String {
    assert!(hex.len() % 2 == 0, "odd hex length");
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("invalid hex"))
        .collect();
    String::from_utf8(bytes).expect("payload is not UTF-8")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The frozen M4 text fixture path is supplied by the verifier. Keep engine
    // creation aligned with the previously committed Engine::open M1 API in the real
    // repository. If M1's syntax differs, update this driver *before* freezing
    // the M4 harness; the implementation agent may not change it afterwards.
    let root = std::env::var("M4_INDEX_DIR")?;
    let config = std::env::var("M4_CONFIG_DIR")?;
    let engine = Engine::open(root, config)?;

    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for raw in stdin.lock().lines() {
        let raw = raw?;
        if raw.is_empty() {
            continue;
        }
        let p: Vec<&str> = raw.split('\t').collect();
        match p[0] {
            "PUT" | "UPDATE" => {
                assert_eq!(p.len(), 3);
                let text = from_hex(p[2]);
                let doc = TextDocument {
                    id: p[1],
                    field: "title",
                    text: &text,
                };
                if p[0] == "PUT" {
                    engine.put_text(doc)?;
                } else {
                    engine.update_text(doc)?;
                }
                writeln!(stdout, "OK")?;
            }
            "DELETE" => {
                assert_eq!(p.len(), 2);
                engine.remove(p[1])?;
                writeln!(stdout, "OK")?;
            }
            "TERM" | "PHRASE" => {
                assert_eq!(p.len(), 4);
                let search_id = p[1];
                let rank_profile = p[2];
                let text = from_hex(p[3]);
                let query = if p[0] == "TERM" {
                    TextQuery::Term { field: "title", text: &text }
                } else {
                    TextQuery::Phrase { field: "title", text: &text }
                };
                let result = engine.search_text(query, rank_profile)?;
                write!(stdout, "RESULT\t{}\t", search_id)?;
                for (i, hit) in result.hits.iter().enumerate() {
                    if i > 0 { write!(stdout, ";")?; }
                    write!(stdout, "{},{}", hit.document_id, hit.score)?;
                }
                writeln!(stdout)?;
            }
            "QUIT" => {
                writeln!(stdout, "OK")?;
                stdout.flush()?;
                return Ok(());
            }
            other => panic!("unknown command {other}"),
        }
        stdout.flush()?;
    }
    Ok(())
}
