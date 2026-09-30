use searchkernel_vespa_engine::{Language, Linguistics, LinguisticsOptions, StemMode};

fn main() {
    let linguistics = Linguistics;

    let defaults = LinguisticsOptions {
        language: Language::English,
        stem_mode: StemMode::Default,
        remove_accents: false,
    };
    let document = linguistics.analyze_document("The quick brown foxes", defaults);
    for term in &document.terms {
        let _: &str = &term.original;
        let _: &str = &term.term;
        let _: usize = term.byte_start;
        let _: usize = term.byte_len;
        let _: u32 = term.position;
    }

    let no_stemming = LinguisticsOptions {
        language: Language::English,
        stem_mode: StemMode::None,
        remove_accents: true,
    };
    let query = linguistics.analyze_query("Café foxes", no_stemming);
    for term in &query.terms {
        let _: &[String] = &term.stems;
        let _: u32 = term.position;
    }
}
