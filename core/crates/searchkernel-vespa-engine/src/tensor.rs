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
        let type_spec = match self {
            Self::Dense { type_spec, .. }
            | Self::Sparse { type_spec, .. }
            | Self::Mixed { type_spec, .. } => type_spec,
        };
        if type_spec.trim().is_empty() {
            return Err("tensor type spec must not be empty".to_owned());
        }
        let finite = match self {
            Self::Dense { values, .. } => values.iter().all(|value| value.is_finite()),
            Self::Sparse { cells, .. } => cells.iter().all(|cell| cell.value.is_finite()),
            Self::Mixed { blocks, .. } => blocks
                .iter()
                .flat_map(|block| block.values.iter())
                .all(|value| value.is_finite()),
        };
        if !finite {
            return Err("tensor cell values must be finite".to_owned());
        }
        Ok(())
    }
}
