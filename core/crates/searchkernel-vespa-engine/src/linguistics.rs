//! The small English linguistics surface used by the Search Kernel wrapper.
//!
//! This follows the pinned Vespa `SimpleTokenizer`, `SimpleNormalizer`,
//! `SimpleTransformer`, and `StemmerImpl` behavior.  In particular, source
//! spans always refer to the unmodified input, while normalization is applied
//! only to the token value.
//!
//! Vespa classifies characters through the pinned JDK's
//! `Character.getType()`. The Rust category table is pinned independently, so
//! the live Java oracle remains the authority when Unicode versions diverge.

mod data;
mod stemmer;

use unicode_general_category::{get_general_category, GeneralCategory};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Languages currently supported by the replacement linguistics layer.
pub enum Language {
    English,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Stemming behavior applied during analysis.
pub enum StemMode {
    None,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Options controlling document and query analysis.
pub struct LinguisticsOptions {
    pub language: Language,
    pub stem_mode: StemMode,
    pub remove_accents: bool,
}

impl Default for LinguisticsOptions {
    fn default() -> Self {
        Self {
            language: Language::English,
            stem_mode: StemMode::Default,
            remove_accents: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// The analyzed terms produced from document text.
pub struct DocumentAnalysis {
    pub terms: Vec<DocumentTerm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One analyzed document term and its source span.
pub struct DocumentTerm {
    pub original: String,
    pub term: String,
    pub byte_start: usize,
    pub byte_len: usize,
    pub position: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// The analyzed terms produced from query text.
pub struct QueryAnalysis {
    pub terms: Vec<QueryTerm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One query position and its possible stems.
pub struct QueryTerm {
    pub stems: Vec<String>,
    pub position: u32,
}

#[derive(Debug, Default)]
/// Vespa-compatible document and query analyzer.
pub struct Linguistics;

impl Linguistics {
    pub fn analyze_document(&self, text: &str, options: LinguisticsOptions) -> DocumentAnalysis {
        let mut terms = Vec::new();
        let mut stemmer = stemmer::KStemmer::default();
        for token in tokenize(text) {
            if !token.class.is_indexable() {
                continue;
            }
            let original = &text[token.start..token.end];
            let term = process_token(original, options, true, &mut stemmer);
            if term.trim().chars().next().is_some() {
                let position = terms.len() as u32;
                terms.push(DocumentTerm {
                    original: original.to_owned(),
                    term,
                    byte_start: token.start,
                    byte_len: token.end - token.start,
                    position,
                });
            }
        }
        DocumentAnalysis { terms }
    }

    pub fn analyze_query(&self, text: &str, options: LinguisticsOptions) -> QueryAnalysis {
        let mut terms = Vec::new();
        let mut stemmer = stemmer::KStemmer::default();
        for token in tokenize(text) {
            if !token.class.is_indexable() {
                continue;
            }

            // Stemmer.stem(String, Language, StemMode, boolean) in the pinned
            // Vespa API delegates to the overload which uses removeAccents=false.
            let term = process_token(&text[token.start..token.end], options, false, &mut stemmer);
            let position = terms.len() as u32;
            terms.push(QueryTerm {
                stems: vec![term],
                position,
            });
        }
        QueryAnalysis { terms }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Token {
    start: usize,
    end: usize,
    class: TokenClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenClass {
    Unknown,
    Space,
    Punctuation,
    Symbol,
    Alphabetic,
    Numeric,
    IndexableSymbol,
}

impl TokenClass {
    fn is_indexable(self) -> bool {
        matches!(
            self,
            Self::Alphabetic | Self::Numeric | Self::IndexableSymbol
        )
    }
}

fn token_class(character: char) -> TokenClass {
    match get_general_category(character) {
        GeneralCategory::NonspacingMark
        | GeneralCategory::SpacingMark
        | GeneralCategory::LetterNumber
        | GeneralCategory::UppercaseLetter
        | GeneralCategory::LowercaseLetter
        | GeneralCategory::TitlecaseLetter
        | GeneralCategory::ModifierLetter
        | GeneralCategory::OtherLetter => TokenClass::Alphabetic,
        GeneralCategory::EnclosingMark
        | GeneralCategory::MathSymbol
        | GeneralCategory::CurrencySymbol
        | GeneralCategory::ModifierSymbol => TokenClass::Symbol,
        GeneralCategory::OtherSymbol => TokenClass::IndexableSymbol,
        GeneralCategory::OtherNumber | GeneralCategory::DecimalNumber => TokenClass::Numeric,
        GeneralCategory::SpaceSeparator
        | GeneralCategory::LineSeparator
        | GeneralCategory::ParagraphSeparator => TokenClass::Space,
        GeneralCategory::DashPunctuation
        | GeneralCategory::OpenPunctuation
        | GeneralCategory::ClosePunctuation
        | GeneralCategory::ConnectorPunctuation
        | GeneralCategory::OtherPunctuation
        | GeneralCategory::InitialPunctuation
        | GeneralCategory::FinalPunctuation => TokenClass::Punctuation,
        GeneralCategory::Control
        | GeneralCategory::Format
        | GeneralCategory::Surrogate
        | GeneralCategory::PrivateUse
        | GeneralCategory::Unassigned => TokenClass::Unknown,
        _ => TokenClass::Unknown,
    }
}

fn tokenize(text: &str) -> Vec<Token> {
    let mut chars = text.char_indices().peekable();
    let Some((start, first)) = chars.next() else {
        return Vec::new();
    };

    let mut token_start = start;
    let mut previous = token_class(first);
    let mut current = previous;
    let mut tokens = Vec::new();

    while let Some(&(next_start, next_char)) = chars.peek() {
        let next = token_class(next_char);
        if at_boundary(previous, next) {
            tokens.push(Token {
                start: token_start,
                end: next_start,
                class: current,
            });
            token_start = next_start;
            previous = next;
            current = next;
        } else {
            if next == TokenClass::Alphabetic {
                current = TokenClass::Alphabetic;
            }
            previous = next;
        }
        chars.next();
    }

    tokens.push(Token {
        start: token_start,
        end: text.len(),
        class: current,
    });
    tokens
}

fn at_boundary(previous: TokenClass, next: TokenClass) -> bool {
    previous == TokenClass::IndexableSymbol
        || next == TokenClass::IndexableSymbol
        || !previous.is_indexable()
        || !next.is_indexable()
}

fn process_token(
    original: &str,
    options: LinguisticsOptions,
    apply_accents: bool,
    kstemmer: &mut stemmer::KStemmer,
) -> String {
    let mut token: String = original.nfkc().collect();
    token = lowercase(&token);
    if apply_accents && options.remove_accents {
        token = accent_drop(&token);
    }
    if options.stem_mode == StemMode::Default {
        token = kstemmer.stem_word(&token);
    }
    token
}

fn lowercase(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for (index, character) in input.char_indices() {
        if character == '\u{03a3}' {
            output.push(if is_final_sigma(input, index) {
                '\u{03c2}'
            } else {
                '\u{03c3}'
            });
        } else if character == '\u{0130}' {
            output.push('i');
        } else {
            output.extend(character.to_lowercase());
        }
    }
    output
}

fn is_final_sigma(input: &str, sigma_start: usize) -> bool {
    let before = &input[..sigma_start];
    let after = &input[sigma_start + '\u{03a3}'.len_utf8()..];

    let preceded_by_cased = before
        .chars()
        .rev()
        .find(|&character| !is_case_ignorable(character))
        .is_some_and(is_cased);
    let followed_by_cased = after
        .chars()
        .find(|&character| !is_case_ignorable(character))
        .is_some_and(is_cased);

    preceded_by_cased && !followed_by_cased
}

fn is_cased(character: char) -> bool {
    matches!(
        get_general_category(character),
        GeneralCategory::UppercaseLetter
            | GeneralCategory::LowercaseLetter
            | GeneralCategory::TitlecaseLetter
    )
}

fn is_case_ignorable(character: char) -> bool {
    matches!(
        get_general_category(character),
        GeneralCategory::NonspacingMark
            | GeneralCategory::EnclosingMark
            | GeneralCategory::Format
            | GeneralCategory::ModifierLetter
            | GeneralCategory::ModifierSymbol
            | GeneralCategory::OtherPunctuation
    )
}

fn accent_drop(input: &str) -> String {
    input
        .nfd()
        .filter(|&character| !(('\u{0300}'..='\u{036f}').contains(&character)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Language, Linguistics, LinguisticsOptions, StemMode};

    fn options(stem_mode: StemMode, remove_accents: bool) -> LinguisticsOptions {
        LinguisticsOptions {
            language: Language::English,
            stem_mode,
            remove_accents,
        }
    }

    #[test]
    fn blank_and_whitespace_input_emit_no_terms() {
        let linguistics = Linguistics;
        assert_eq!(
            linguistics
                .analyze_document("  \t\n  ", options(StemMode::Default, false))
                .terms
                .len(),
            0
        );
        assert_eq!(
            linguistics
                .analyze_query("", options(StemMode::Default, false))
                .terms
                .len(),
            0
        );
    }

    #[test]
    fn tokenizer_preserves_order_and_repetitions() {
        let analysis =
            Linguistics.analyze_document("fox fox C++ foo-bar", options(StemMode::None, false));
        let terms: Vec<_> = analysis
            .terms
            .iter()
            .map(|term| term.term.as_str())
            .collect();
        assert_eq!(terms, ["fox", "fox", "c", "foo", "bar"]);
        assert_eq!(
            analysis
                .terms
                .iter()
                .map(|term| term.position)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3, 4]
        );
    }

    #[test]
    fn normalization_and_accents_are_document_side_operations() {
        let linguistics = Linguistics;
        let retained = linguistics.analyze_document("ＦＯＯ Café", options(StemMode::None, false));
        assert_eq!(retained.terms[0].term, "foo");
        assert_eq!(retained.terms[1].term, "café");

        let removed = linguistics.analyze_document("ＦＯＯ Café", options(StemMode::None, true));
        assert_eq!(removed.terms[1].term, "cafe");

        let query = linguistics.analyze_query("Café", options(StemMode::None, true));
        assert_eq!(query.terms[0].stems, ["café"]);
    }

    #[test]
    fn lowercase_matches_english_contextual_sigma_rules() {
        assert_eq!(super::lowercase("ΟΣ"), "ος");
        assert_eq!(super::lowercase("ΟΣΑ"), "οσα");
        assert_eq!(super::lowercase("Ο\u{301}Σ"), "ο\u{301}ς");
        assert_eq!(super::lowercase("Σ"), "σ");
    }

    #[test]
    fn spans_are_utf8_offsets_into_the_original() {
        let text = "😀 foxes café";
        let analysis = Linguistics.analyze_document(text, options(StemMode::Default, false));
        for term in &analysis.terms {
            assert_eq!(
                &text[term.byte_start..term.byte_start + term.byte_len],
                term.original
            );
        }
        assert_eq!(analysis.terms[0].byte_len, 4);
        assert_eq!(analysis.terms[1].byte_start, 5);
    }
}
