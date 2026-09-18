//! Frozen parser complexity limits.

pub const MAX_EXPRESSION_BYTES: usize = 65_536;
pub const MAX_TOKENS: usize = 16_384;
pub const MAX_NESTING_DEPTH: usize = 64;
pub const MAX_AST_NODES: usize = 4_096;
pub const MAX_IN_VALUES: usize = 1_024;
pub const MAX_PARAMETER_REFERENCES: usize = 1_024;
