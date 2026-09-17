use crate::index::Index;
use crate::schema::Analyzer;

/// A fluent request for linguistic analysis of text.
pub struct AnalyzeBuilder {
    pub(crate) index: Index,
    pub(crate) target: Option<AnalyzeTarget>,
    pub(crate) field_name: Option<String>,
    pub(crate) text: String,
    pub(crate) language: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AnalyzeTarget {
    Analyzer(Analyzer),
    Field,
}

impl AnalyzeBuilder {
    pub(crate) fn new(index: Index, text: impl Into<String>) -> Self {
        Self {
            index,
            target: None,
            field_name: None,
            text: text.into(),
            language: None,
        }
    }

    /// Analyze using the analyzer configured for `field`.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.target = Some(AnalyzeTarget::Field);
        self.field_name = Some(field.into());
        self
    }

    /// Analyze using an explicit analyzer.
    pub fn analyzer(mut self, analyzer: Analyzer) -> Self {
        self.target = Some(AnalyzeTarget::Analyzer(analyzer));
        self.field_name = None;
        self
    }

    /// Set the language hint passed to the analyzer.
    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Execute the analysis request.
    pub fn execute(self) -> crate::Result<AnalyzeResult> {
        self.index.execute_analyze(&self)
    }
}

/// One token produced by linguistic analysis.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct AnalyzedToken {
    pub surface: String,
    pub term: String,
    pub position: u32,
    pub start_byte: u32,
    pub end_byte: u32,
}

/// The tokens produced by an analysis request.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct AnalyzeResult {
    pub tokens: Vec<AnalyzedToken>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{Index, IndexRuntime};
    use crate::schema::{Field, IndexDefinition};
    use searchkernel_testkit::RecordingBackend;
    use std::sync::Arc;

    fn test_index() -> (IndexRuntime, Index) {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::text("title")),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");
        let index = runtime.index();
        (runtime, index)
    }

    #[test]
    fn analyze_builder_switches_between_field_and_explicit_analyzer() {
        let (_runtime, index) = test_index();
        let field = AnalyzeBuilder::new(index.clone(), "Running")
            .field("title")
            .language("EN-us");
        assert_eq!(field.target, Some(AnalyzeTarget::Field));
        assert_eq!(field.field_name.as_deref(), Some("title"));
        assert_eq!(field.language.as_deref(), Some("EN-us"));

        let explicit = field.analyzer(Analyzer::standard_v1());
        assert_eq!(
            explicit.target,
            Some(AnalyzeTarget::Analyzer(Analyzer::standard_v1()))
        );
        assert_eq!(explicit.field_name, None);
        assert_eq!(explicit.text, "Running");
    }

    #[test]
    fn analyze_builder_owns_index_handle() {
        let builder = {
            let (_runtime, index) = test_index();
            index.analyze("Running").analyzer(Analyzer::standard_v1())
        };

        assert!(matches!(
            builder.execute(),
            Err(error) if error.code() == crate::ErrorCode::Unsupported
        ));
    }
}
