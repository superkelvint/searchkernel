//! Tensor values used by query inputs and tensor document fields.

#[derive(Clone, Debug, PartialEq)]
pub struct TensorAddress {
    pub dimension: String,
    pub label: String,
}

impl TensorAddress {
    pub fn new(dimension: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            dimension: dimension.into(),
            label: label.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TensorCell {
    pub address: Vec<TensorAddress>,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TensorBlock {
    pub address: Vec<TensorAddress>,
    pub values: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TensorValue {
    Dense {
        type_spec: String,
        values: Vec<f64>,
    },
    Sparse {
        type_spec: String,
        cells: Vec<TensorCell>,
    },
    Mixed {
        type_spec: String,
        blocks: Vec<TensorBlock>,
    },
}

impl TensorValue {
    pub fn dense(type_spec: impl Into<String>, values: impl Into<Vec<f64>>) -> Self {
        Self::Dense {
            type_spec: type_spec.into(),
            values: values.into(),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::Dense { type_spec, values } => {
                validate_dense(type_spec, values).map_err(|e| e.message)
            }
            Self::Sparse { type_spec, cells } => {
                require_type_spec(type_spec)?;
                if !cells.iter().all(|cell| cell.value.is_finite()) {
                    return Err("tensor cell values must be finite".to_owned());
                }
                Ok(())
            }
            Self::Mixed { type_spec, blocks } => {
                require_type_spec(type_spec)?;
                if !blocks
                    .iter()
                    .flat_map(|block| block.values.iter())
                    .all(|value| value.is_finite())
                {
                    return Err("tensor cell values must be finite".to_owned());
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct DenseTensorValidationError {
    pub(crate) detail_code: &'static str,
    pub(crate) message: String,
}

fn require_type_spec(type_spec: &str) -> Result<(), String> {
    if type_spec.trim().is_empty() {
        Err("tensor type spec must not be empty".to_owned())
    } else {
        Ok(())
    }
}

pub(crate) fn validate_dense(
    type_spec: &str,
    values: &[f64],
) -> Result<(), DenseTensorValidationError> {
    let dimension_size = parse_single_dense_dimension(type_spec)?;
    if dimension_size != values.len() {
        return Err(DenseTensorValidationError {
            detail_code: "tensor_cell_count_mismatch",
            message: "dense tensor cell count does not match its type spec".to_owned(),
        });
    }
    if !values.iter().all(|value| value.is_finite()) {
        return Err(DenseTensorValidationError {
            detail_code: "tensor_non_finite_cell",
            message: "tensor cell values must be finite".to_owned(),
        });
    }
    Ok(())
}

fn parse_single_dense_dimension(type_spec: &str) -> Result<usize, DenseTensorValidationError> {
    let spec = type_spec.trim();
    let Some(rest) = spec.strip_prefix("tensor") else {
        return malformed();
    };
    let rest = if let Some(rest) = rest.strip_prefix('<') {
        let Some((cell_type, tail)) = rest.split_once('>') else {
            return malformed();
        };
        if !matches!(cell_type, "double" | "float" | "bfloat16" | "int8") {
            return malformed();
        }
        tail
    } else {
        rest
    };
    let Some(dimensions) = rest.strip_prefix('(').and_then(|s| s.strip_suffix(')')) else {
        return malformed();
    };
    let dimensions = dimensions.trim();
    if dimensions.is_empty() {
        return malformed();
    }
    let parts: Vec<_> = dimensions.split(',').map(str::trim).collect();
    if parts.iter().any(|part| part.is_empty()) {
        return malformed();
    }
    let mut indexed_size = None;
    let mut indexed_count = 0;
    let mut mapped_count = 0;
    for dimension in parts {
        if let Some(name) = dimension.strip_suffix("{}") {
            if !is_dimension_name(name.trim()) {
                return malformed();
            }
            mapped_count += 1;
            continue;
        }
        let Some(open) = dimension.rfind('[') else {
            return malformed();
        };
        let Some(size_text) = dimension[open + 1..].strip_suffix(']') else {
            return malformed();
        };
        let name = dimension[..open].trim();
        if !is_dimension_name(name) {
            return malformed();
        }
        let Ok(size) = size_text.parse::<usize>() else {
            return malformed();
        };
        if size == 0 {
            return malformed();
        }
        indexed_count += 1;
        indexed_size = Some(size);
    }
    if indexed_count != 1 || mapped_count != 0 {
        return unsupported_shape();
    }
    Ok(indexed_size.expect("one indexed dimension was counted"))
}

fn is_dimension_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn malformed<T>() -> Result<T, DenseTensorValidationError> {
    Err(DenseTensorValidationError {
        detail_code: "tensor_type_spec_invalid",
        message: "tensor type specification is malformed".to_owned(),
    })
}

fn unsupported_shape<T>() -> Result<T, DenseTensorValidationError> {
    Err(DenseTensorValidationError {
        detail_code: "tensor_shape_unsupported",
        message: "dense tensors must have exactly one indexed dimension and no mapped dimensions"
            .to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::validate_dense;

    #[test]
    fn dense_type_validation_matches_native_error_categories() {
        for (type_spec, values, expected) in [
            ("not-a-tensor", vec![1.0, 0.0], "tensor_type_spec_invalid"),
            ("tensor<float>(x[0])", vec![], "tensor_type_spec_invalid"),
            (
                "tensor<float>(x[2],y[1])",
                vec![1.0, 0.0],
                "tensor_shape_unsupported",
            ),
            (
                "tensor<float>(m{},x[2])",
                vec![1.0, 0.0],
                "tensor_shape_unsupported",
            ),
            (
                "tensor<float>(x[3])",
                vec![1.0, 0.0],
                "tensor_cell_count_mismatch",
            ),
            (
                "tensor<float>(x[2])",
                vec![f64::NAN, 0.0],
                "tensor_non_finite_cell",
            ),
        ] {
            let error = validate_dense(type_spec, &values).expect_err("invalid tensor accepted");
            assert_eq!(error.detail_code, expected, "{type_spec}");
        }
        for type_spec in [
            "tensor(x[2])",
            "tensor<double>(x[2])",
            "tensor<float>(x[2])",
            "tensor<bfloat16>(x[2])",
            "tensor<int8>(x[2])",
        ] {
            validate_dense(type_spec, &[1.0, 0.0]).expect("supported dense tensor rejected");
        }
    }
}
