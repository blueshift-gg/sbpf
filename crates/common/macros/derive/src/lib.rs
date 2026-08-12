use {
    proc_macro::TokenStream,
    syn::{DeriveInput, parse_macro_input},
};

#[proc_macro_derive(OpcodeTable, attributes(opcode))]
pub fn derive_opcode_table(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    sbpf_common_tablegen::derive_opcode_table(&input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_derive(OpcodeGroup, attributes(group, handlers))]
pub fn derive_opcode_group(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    sbpf_common_tablegen::derive_opcode_group(&input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
