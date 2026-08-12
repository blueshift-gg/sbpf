pub mod error;
pub mod expand;
pub mod ir;
pub mod parse;
pub mod validate;

pub use {
    expand::{expand_opcode_group, expand_opcode_table},
    ir::{GroupDef, HandlerTypePaths, OpcodeArch, OpcodeDef, OpcodeGroupDef, OpcodeTableDef},
    parse::{parse_opcode_group, parse_opcode_table},
    validate::{validate_opcode_group, validate_opcode_table},
};

/// Trait for OpcodeTable metadata, group membership, and byte conversions.
pub trait OpcodeTable: Copy + Sized + 'static {
    type Group: OpcodeGroup;

    fn to_str(&self) -> &'static str;
    fn group(self) -> Self::Group;
    fn try_from_sbpf_v3(opcode: u8) -> Result<Self, OpcodeError>;
    fn all() -> &'static [Self];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpcodeError {
    InvalidOpcode { byte: u8 },
}

impl core::fmt::Display for OpcodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            OpcodeError::InvalidOpcode { byte } => {
                write!(f, "invalid opcode: 0x{byte:02x}")
            }
        }
    }
}

impl std::error::Error for OpcodeError {}

/// Trait for OpcodeGroup metadata and decode/validate/execute handler routing.
pub trait OpcodeGroup: HandlerTypes + Copy + Sized + 'static {
    fn title(self) -> &'static str;
    fn description(self) -> &'static str;
    fn decode_fn(self) -> Self::DecodeFn;
    fn validate_fn(self) -> Self::ValidateFn;
    fn execute_fn(self) -> Self::ExecuteFn;
    fn all() -> &'static [Self];
}

/// Handler types for an `OpcodeGroup`.
pub trait HandlerTypes {
    type DecodeFn: Copy;
    type ValidateFn: Copy;
    type ExecuteFn: Copy;
}

/// Parse, validate and expand an `OpcodeTable`.
pub fn derive_opcode_table(input: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let table = parse_opcode_table(input)?;
    validate_opcode_table(&table)?;
    Ok(expand_opcode_table(&table))
}

/// Parse, validate and expand an `OpcodeGroup`.
pub fn derive_opcode_group(input: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let table = parse_opcode_group(input)?;
    validate_opcode_group(&table)?;
    Ok(expand_opcode_group(&table))
}

#[cfg(test)]
mod tests;
