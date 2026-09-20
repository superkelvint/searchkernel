//! Idiomatic, Vespa-facing document and mutation values.

use crate::linguistics::DocumentAnalysis;
use crate::tensor::TensorValue;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum ScalarValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

impl From<String> for ScalarValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for ScalarValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<i64> for ScalarValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<i32> for ScalarValue {
    fn from(value: i32) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<f64> for ScalarValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<f32> for ScalarValue {
    fn from(value: f32) -> Self {
        Self::Float(f64::from(value))
    }
}

impl From<bool> for ScalarValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl IntoScalarValue for ScalarValue {
    fn into_scalar_value(self) -> ScalarValue {
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ArrayValue {
    Strings(Vec<String>),
    Integers(Vec<i64>),
    Floats(Vec<f64>),
    Booleans(Vec<bool>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PositionValue {
    pub latitude: f64,
    pub longitude: f64,
}

impl PositionValue {
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PredicateValue {
    pub features: BTreeMap<String, String>,
}

impl PredicateValue {
    pub fn new(features: BTreeMap<String, String>) -> Self {
        Self { features }
    }
}

impl From<Vec<String>> for ArrayValue {
    fn from(value: Vec<String>) -> Self {
        Self::Strings(value)
    }
}

impl From<Vec<i64>> for ArrayValue {
    fn from(value: Vec<i64>) -> Self {
        Self::Integers(value)
    }
}

impl From<Vec<f64>> for ArrayValue {
    fn from(value: Vec<f64>) -> Self {
        Self::Floats(value)
    }
}

impl From<Vec<bool>> for ArrayValue {
    fn from(value: Vec<bool>) -> Self {
        Self::Booleans(value)
    }
}

impl From<Vec<String>> for FieldValue {
    fn from(value: Vec<String>) -> Self {
        Self::Array(ArrayValue::Strings(value))
    }
}

impl From<Vec<i64>> for FieldValue {
    fn from(value: Vec<i64>) -> Self {
        Self::Array(ArrayValue::Integers(value))
    }
}

impl From<Vec<f64>> for FieldValue {
    fn from(value: Vec<f64>) -> Self {
        Self::Array(ArrayValue::Floats(value))
    }
}

impl From<Vec<bool>> for FieldValue {
    fn from(value: Vec<bool>) -> Self {
        Self::Array(ArrayValue::Booleans(value))
    }
}

/// A field value whose array element type is encoded by the variant.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    Scalar(ScalarValue),
    Array(ArrayValue),
    Position(PositionValue),
    Predicate(PredicateValue),
    Tensor(TensorValue),
}

pub trait IntoScalarValue {
    fn into_scalar_value(self) -> ScalarValue;
}

impl IntoScalarValue for String {
    fn into_scalar_value(self) -> ScalarValue {
        ScalarValue::String(self)
    }
}

impl IntoScalarValue for &str {
    fn into_scalar_value(self) -> ScalarValue {
        ScalarValue::String(self.to_owned())
    }
}

impl IntoScalarValue for i64 {
    fn into_scalar_value(self) -> ScalarValue {
        ScalarValue::Integer(self)
    }
}

impl IntoScalarValue for i32 {
    fn into_scalar_value(self) -> ScalarValue {
        ScalarValue::Integer(i64::from(self))
    }
}

impl IntoScalarValue for f64 {
    fn into_scalar_value(self) -> ScalarValue {
        ScalarValue::Float(self)
    }
}

impl IntoScalarValue for f32 {
    fn into_scalar_value(self) -> ScalarValue {
        ScalarValue::Float(f64::from(self))
    }
}

impl IntoScalarValue for bool {
    fn into_scalar_value(self) -> ScalarValue {
        ScalarValue::Boolean(self)
    }
}

pub trait IntoFieldValue {
    fn into_field_value(self) -> FieldValue;
}

impl IntoFieldValue for FieldValue {
    fn into_field_value(self) -> FieldValue {
        self
    }
}

impl IntoFieldValue for TensorValue {
    fn into_field_value(self) -> FieldValue {
        FieldValue::Tensor(self)
    }
}

impl IntoFieldValue for PositionValue {
    fn into_field_value(self) -> FieldValue {
        FieldValue::Position(self)
    }
}

impl IntoFieldValue for PredicateValue {
    fn into_field_value(self) -> FieldValue {
        FieldValue::Predicate(self)
    }
}

impl<T: IntoScalarValue> IntoFieldValue for T {
    fn into_field_value(self) -> FieldValue {
        FieldValue::Scalar(self.into_scalar_value())
    }
}

impl<T: IntoScalarValue> IntoFieldValue for Vec<T> {
    fn into_field_value(self) -> FieldValue {
        FieldValue::Array(array_from_scalars(self))
    }
}

impl<T: IntoScalarValue + Clone> IntoFieldValue for &[T] {
    fn into_field_value(self) -> FieldValue {
        FieldValue::Array(array_from_scalars(self.to_vec()))
    }
}

impl<T: IntoScalarValue, const N: usize> IntoFieldValue for [T; N] {
    fn into_field_value(self) -> FieldValue {
        FieldValue::Array(array_from_scalars(self.into_iter().collect()))
    }
}

fn array_from_scalars<T: IntoScalarValue>(values: Vec<T>) -> ArrayValue {
    let mut values = values.into_iter().map(IntoScalarValue::into_scalar_value);
    match values.next() {
        None => ArrayValue::Strings(Vec::new()),
        Some(ScalarValue::String(first)) => ArrayValue::Strings(
            std::iter::once(first)
                .chain(values.map(|value| match value {
                    ScalarValue::String(value) => value,
                    _ => unreachable!("typed conversion produced a heterogeneous array"),
                }))
                .collect(),
        ),
        Some(ScalarValue::Integer(first)) => ArrayValue::Integers(
            std::iter::once(first)
                .chain(values.map(|value| match value {
                    ScalarValue::Integer(value) => value,
                    _ => unreachable!("typed conversion produced a heterogeneous array"),
                }))
                .collect(),
        ),
        Some(ScalarValue::Float(first)) => ArrayValue::Floats(
            std::iter::once(first)
                .chain(values.map(|value| match value {
                    ScalarValue::Float(value) => value,
                    _ => unreachable!("typed conversion produced a heterogeneous array"),
                }))
                .collect(),
        ),
        Some(ScalarValue::Boolean(first)) => ArrayValue::Booleans(
            std::iter::once(first)
                .chain(values.map(|value| match value {
                    ScalarValue::Boolean(value) => value,
                    _ => unreachable!("typed conversion produced a heterogeneous array"),
                }))
                .collect(),
        ),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    id: String,
    fields: BTreeMap<String, FieldValue>,
    text_analysis: BTreeMap<String, DocumentAnalysis>,
}

impl Document {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            fields: BTreeMap::new(),
            text_analysis: BTreeMap::new(),
        }
    }

    pub fn field(mut self, name: impl Into<String>, value: impl IntoFieldValue) -> Self {
        self.fields.insert(name.into(), value.into_field_value());
        self
    }

    /// Attach Vespa-compatible term annotations to a string field. The
    /// field's schema still determines whether these annotations are used as
    /// full-text indexing data or ignored as an exact string value.
    pub fn annotate_text(mut self, name: impl Into<String>, analysis: DocumentAnalysis) -> Self {
        self.text_analysis.insert(name.into(), analysis);
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn fields(&self) -> &BTreeMap<String, FieldValue> {
        &self.fields
    }

    pub(crate) fn text_analysis(&self) -> &BTreeMap<String, DocumentAnalysis> {
        &self.text_analysis
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArithmeticOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldUpdate {
    Assign(FieldValue),
    Clear,
    Arithmetic {
        operator: ArithmeticOperator,
        operand: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentUpdate {
    id: String,
    fields: BTreeMap<String, FieldUpdate>,
    text_analysis: BTreeMap<String, DocumentAnalysis>,
}

impl DocumentUpdate {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            fields: BTreeMap::new(),
            text_analysis: BTreeMap::new(),
        }
    }

    pub fn assign(mut self, name: impl Into<String>, value: impl IntoFieldValue) -> Self {
        self.fields
            .insert(name.into(), FieldUpdate::Assign(value.into_field_value()));
        self
    }

    pub fn clear(mut self, name: impl Into<String>) -> Self {
        self.fields.insert(name.into(), FieldUpdate::Clear);
        self
    }

    pub fn arithmetic(
        mut self,
        name: impl Into<String>,
        operator: ArithmeticOperator,
        operand: f64,
    ) -> Self {
        self.fields
            .insert(name.into(), FieldUpdate::Arithmetic { operator, operand });
        self
    }

    /// Attach Vespa-compatible term annotations to a string assignment.
    pub fn annotate_text(mut self, name: impl Into<String>, analysis: DocumentAnalysis) -> Self {
        self.text_analysis.insert(name.into(), analysis);
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn fields(&self) -> &BTreeMap<String, FieldUpdate> {
        &self.fields
    }

    pub(crate) fn text_analysis(&self) -> &BTreeMap<String, DocumentAnalysis> {
        &self.text_analysis
    }
}

#[cfg(test)]
mod tests {
    use super::{ArrayValue, Document, DocumentUpdate, FieldUpdate, FieldValue, ScalarValue};
    use crate::{DocumentAnalysis, DocumentTerm};

    #[test]
    fn builder_converts_scalars_and_homogeneous_arrays() {
        let document = Document::new("sku-42")
            .field("title", "Wireless Headphones")
            .field("price", 149_i64)
            .field("active", true)
            .field("tags", ["audio", "wireless"]);
        assert_eq!(
            document.fields()["price"],
            FieldValue::Scalar(ScalarValue::Integer(149))
        );
        assert_eq!(
            document.fields()["tags"],
            FieldValue::Array(ArrayValue::Strings(vec!["audio".into(), "wireless".into()]))
        );
    }

    #[test]
    fn conversions_cover_all_scalar_and_array_shapes() {
        assert_eq!(
            ScalarValue::from(String::from("owned")),
            ScalarValue::String("owned".into())
        );
        assert_eq!(
            ScalarValue::from("borrowed"),
            ScalarValue::String("borrowed".into())
        );
        assert_eq!(ScalarValue::from(-7_i64), ScalarValue::Integer(-7));
        assert_eq!(ScalarValue::from(-3_i32), ScalarValue::Integer(-3));
        assert_eq!(ScalarValue::from(2.5_f64), ScalarValue::Float(2.5));
        assert_eq!(ScalarValue::from(1.25_f32), ScalarValue::Float(1.25));
        assert_eq!(ScalarValue::from(true), ScalarValue::Boolean(true));

        assert_eq!(
            ArrayValue::from(vec!["one".to_owned(), "two".to_owned()]),
            ArrayValue::Strings(vec!["one".into(), "two".into()])
        );
        assert_eq!(
            ArrayValue::from(vec![1_i64, 2]),
            ArrayValue::Integers(vec![1, 2])
        );
        assert_eq!(
            ArrayValue::from(vec![1.5_f64, 2.5]),
            ArrayValue::Floats(vec![1.5, 2.5])
        );
        assert_eq!(
            ArrayValue::from(vec![true, false]),
            ArrayValue::Booleans(vec![true, false])
        );

        let integer_slice = [4_i32, 5_i32];
        let document = Document::new("all-shapes")
            .field("string", String::from("value"))
            .field("integer", -8_i32)
            .field("float", 3.25_f32)
            .field("boolean", false)
            .field("strings", vec!["a".to_owned(), "b".to_owned()])
            .field("integers", vec![10_i64, 11])
            .field("floats", vec![4.5_f64, 5.5])
            .field("booleans", vec![true, false])
            .field("empty", Vec::<i64>::new())
            .field("slice", &integer_slice[..])
            .field("array", [true, false])
            .field("explicit", FieldValue::from(vec!["x".to_owned()]));

        assert_eq!(
            document.fields()["string"],
            FieldValue::Scalar(ScalarValue::String("value".into()))
        );
        assert_eq!(
            document.fields()["integer"],
            FieldValue::Scalar(ScalarValue::Integer(-8))
        );
        assert_eq!(
            document.fields()["float"],
            FieldValue::Scalar(ScalarValue::Float(3.25))
        );
        assert_eq!(
            document.fields()["boolean"],
            FieldValue::Scalar(ScalarValue::Boolean(false))
        );
        assert_eq!(
            document.fields()["integers"],
            FieldValue::Array(ArrayValue::Integers(vec![10, 11]))
        );
        assert_eq!(
            document.fields()["floats"],
            FieldValue::Array(ArrayValue::Floats(vec![4.5, 5.5]))
        );
        assert_eq!(
            document.fields()["booleans"],
            FieldValue::Array(ArrayValue::Booleans(vec![true, false]))
        );
        assert_eq!(
            document.fields()["empty"],
            FieldValue::Array(ArrayValue::Strings(Vec::new()))
        );
        assert_eq!(
            document.fields()["slice"],
            FieldValue::Array(ArrayValue::Integers(vec![4, 5]))
        );
        assert_eq!(
            document.fields()["array"],
            FieldValue::Array(ArrayValue::Booleans(vec![true, false]))
        );
    }

    #[test]
    fn document_and_update_annotations_preserve_their_entries() {
        let analysis = DocumentAnalysis {
            terms: vec![DocumentTerm {
                original: "Running".into(),
                term: "run".into(),
                byte_start: 0,
                byte_len: 7,
                position: 1,
            }],
        };
        let document = Document::new("annotated")
            .field("title", "Running")
            .annotate_text("title", analysis.clone());
        assert_eq!(document.text_analysis()["title"], analysis);

        let update = DocumentUpdate::new("annotated")
            .assign("title", "Running")
            .clear("obsolete")
            .annotate_text("title", analysis.clone());
        assert_eq!(update.id(), "annotated");
        assert_eq!(
            update.fields()["title"],
            FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::String("Running".into())))
        );
        assert_eq!(update.fields()["obsolete"], FieldUpdate::Clear);
        assert_eq!(update.text_analysis()["title"], analysis);
    }
}
