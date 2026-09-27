use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
/// The stable identifier of a document in an index.
pub struct DocumentId(String);

impl DocumentId {
    /// Borrow the identifier as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DocumentId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for DocumentId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&String> for DocumentId {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

impl From<&DocumentId> for DocumentId {
    fn from(value: &DocumentId) -> Self {
        value.clone()
    }
}

impl AsRef<str> for DocumentId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for DocumentId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy)]
struct FiniteFloatSerializer;

struct FiniteFloatCompound;

impl serde::Serializer for FiniteFloatSerializer {
    type Ok = ();
    type Error = serde_json::Error;
    type SerializeSeq = FiniteFloatCompound;
    type SerializeTuple = FiniteFloatCompound;
    type SerializeTupleStruct = FiniteFloatCompound;
    type SerializeTupleVariant = FiniteFloatCompound;
    type SerializeMap = FiniteFloatCompound;
    type SerializeStruct = FiniteFloatCompound;
    type SerializeStructVariant = FiniteFloatCompound;

    fn serialize_f32(self, value: f32) -> Result<(), Self::Error> {
        self.serialize_f64(f64::from(value))
    }
    fn serialize_f64(self, value: f64) -> Result<(), Self::Error> {
        if value.is_finite() {
            Ok(())
        } else {
            Err(serde::ser::Error::custom("non-finite floating-point value"))
        }
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        value.serialize(self)
    }
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), Self::Error> {
        value.serialize(self)
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(FiniteFloatCompound)
    }
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(FiniteFloatCompound)
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(FiniteFloatCompound)
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(FiniteFloatCompound)
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(FiniteFloatCompound)
    }
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(FiniteFloatCompound)
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(FiniteFloatCompound)
    }

    fn serialize_bool(self, _v: bool) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i8(self, _v: i8) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i16(self, _v: i16) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i32(self, _v: i32) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i64(self, _v: i64) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i128(self, _v: i128) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u8(self, _v: u8) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u16(self, _v: u16) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u32(self, _v: u32) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u64(self, _v: u64) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u128(self, _v: u128) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_char(self, _v: char) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_str(self, _v: &str) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_bytes(self, _v: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_none(self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_unit(self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeSeq for FiniteFloatCompound {
    type Ok = ();
    type Error = serde_json::Error;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(FiniteFloatSerializer)
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl serde::ser::SerializeTuple for FiniteFloatCompound {
    type Ok = ();
    type Error = serde_json::Error;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(FiniteFloatSerializer)
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl serde::ser::SerializeTupleStruct for FiniteFloatCompound {
    type Ok = ();
    type Error = serde_json::Error;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(FiniteFloatSerializer)
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl serde::ser::SerializeTupleVariant for FiniteFloatCompound {
    type Ok = ();
    type Error = serde_json::Error;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(FiniteFloatSerializer)
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl serde::ser::SerializeMap for FiniteFloatCompound {
    type Ok = ();
    type Error = serde_json::Error;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Self::Error> {
        key.serialize(FiniteFloatSerializer)
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(FiniteFloatSerializer)
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl serde::ser::SerializeStruct for FiniteFloatCompound {
    type Ok = ();
    type Error = serde_json::Error;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        _key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        value.serialize(FiniteFloatSerializer)
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl serde::ser::SerializeStructVariant for FiniteFloatCompound {
    type Ok = ();
    type Error = serde_json::Error;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        _key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        value.serialize(FiniteFloatSerializer)
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(crate) fn serialize<T: Serialize>(value: &T) -> Result<serde_json::Value, serde_json::Error> {
    value.serialize(FiniteFloatSerializer)?;
    serde_json::to_value(value)
}

pub(crate) type PendingValue = std::result::Result<serde_json::Value, String>;

pub(crate) fn pending<T: Serialize>(value: &T) -> PendingValue {
    serialize(value).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingSerialize;

    impl serde::Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom(
                "intentional serialization failure",
            ))
        }
    }

    #[test]
    fn document_ids_are_owned_and_borrowable() {
        let source = String::from("id:ns:type::1");
        let id = DocumentId::from(&source);
        assert_eq!(id.as_str(), source);
        assert_eq!(id.as_ref(), "id:ns:type::1");
        assert_eq!(id.to_string(), "id:ns:type::1");
        assert_eq!(DocumentId::from(&id), id);
    }

    #[test]
    fn pending_values_preserve_serialization_failures() {
        assert_eq!(pending(&"value"), Ok(serde_json::json!("value")));
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let error = pending(&value).expect_err("non-finite float must be rejected");
            assert!(error.contains("non-finite floating-point value"));
        }
        assert_eq!(pending(&Option::<f64>::None), Ok(serde_json::Value::Null));
        let nested = serde_json::json!({"value": null});
        assert_eq!(pending(&nested), Ok(nested));
        let error = pending(&FailingSerialize).expect_err("serializer failure must be retained");
        assert!(error.contains("intentional serialization failure"));
    }
}
