use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::fmt;

/// A dense tensor value accepted by document writes and query inputs.
///
/// The public JSON-based document API carries this value through an explicit
/// tagged representation at the IR boundary; callers do not need to know
/// about FlatBuffers or the native tensor ABI.
#[derive(Clone, Debug, PartialEq)]
pub struct Tensor {
    type_spec: String,
    values: Vec<f64>,
}

impl Tensor {
    pub fn dense(type_spec: impl Into<String>, values: impl Into<Vec<f64>>) -> Self {
        Self {
            type_spec: type_spec.into(),
            values: values.into(),
        }
    }

    pub fn type_spec(&self) -> &str {
        &self.type_spec
    }

    pub fn values(&self) -> &[f64] {
        &self.values
    }
}

impl Serialize for Tensor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut object = serializer.serialize_struct("SearchKernelTensor", 1)?;
        object.serialize_field(
            "$searchkernel_tensor",
            &TensorPayload {
                type_spec: &self.type_spec,
                values: &self.values,
            },
        )?;
        object.end()
    }
}

struct TensorPayload<'a> {
    type_spec: &'a str,
    values: &'a [f64],
}

impl Serialize for TensorPayload<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut object = serializer.serialize_struct("TensorPayload", 2)?;
        object.serialize_field("type_spec", self.type_spec)?;
        object.serialize_field("dense", self.values)?;
        object.end()
    }
}

impl<'de> serde::Deserialize<'de> for Tensor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TensorVisitor;

        impl<'de> Visitor<'de> for TensorVisitor {
            type Value = Tensor;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a Search Kernel dense tensor object")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut payload = None;
                while let Some(key) = map.next_key::<String>()? {
                    if key == "$searchkernel_tensor" {
                        if payload.is_some() {
                            return Err(de::Error::duplicate_field("$searchkernel_tensor"));
                        }
                        payload = Some(map.next_value::<TensorPayloadOwned>()?);
                    } else {
                        let _: serde::de::IgnoredAny = map.next_value()?;
                    }
                }
                let payload =
                    payload.ok_or_else(|| de::Error::missing_field("$searchkernel_tensor"))?;
                Ok(Tensor::dense(payload.type_spec, payload.dense))
            }
        }

        deserializer.deserialize_map(TensorVisitor)
    }
}

#[derive(serde::Deserialize)]
struct TensorPayloadOwned {
    type_spec: String,
    dense: Vec<f64>,
}
