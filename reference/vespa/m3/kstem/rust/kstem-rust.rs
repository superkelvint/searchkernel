#[path = "../../../../../core/crates/searchkernel-vespa-engine/src/linguistics/data.rs"]
mod data;
#[path = "../../../../../core/crates/searchkernel-vespa-engine/src/linguistics/stemmer.rs"]
mod stemmer;

use std::io::{self, BufRead, BufWriter, Write};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let term = line?;
        if term.contains('\t') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "corpus terms may not contain tabs",
            ));
        }
        writeln!(out, "{}\t{}", term, stemmer::stem(&term))?;
    }
    out.flush()
}
