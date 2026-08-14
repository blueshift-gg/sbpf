use {
    crate::ir::{OpcodeGroupDef, OpcodeTableDef},
    proc_macro2::TokenStream,
    quote::quote,
    syn::Path,
};

/// Expand `#[derive(OpcodeTable)]` into conversions and the `OpcodeTable` trait impl.
pub fn expand_opcode_table(def: &OpcodeTableDef) -> TokenStream {
    let from_str = expand_from_str(def);
    let display = expand_display(def);
    let try_from_u8 = expand_try_from_u8(def);
    let from_opcode_u8 = expand_from_opcode_for_u8(def);
    let table_impl = expand_opcode_table_trait(def);

    quote! {
        #from_str
        #display
        #try_from_u8
        #from_opcode_u8
        #table_impl
    }
}

fn expand_from_str(def: &OpcodeTableDef) -> TokenStream {
    let enum_name = &def.enum_name;
    let mut arms = Vec::new();

    for op in &def.opcodes {
        if !op.is_from_str_target(&def.opcodes) {
            continue;
        }
        let mnemonic = &op.mnemonic;
        let variant = &op.variant;
        arms.push(quote! {
            #mnemonic => Ok(#enum_name::#variant)
        });
    }

    quote! {
        impl ::core::str::FromStr for #enum_name {
            type Err = &'static str;

            fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                match s.to_lowercase().as_str() {
                    #(#arms,)*
                    _ => Err("Invalid opcode"),
                }
            }
        }
    }
}

fn expand_display(def: &OpcodeTableDef) -> TokenStream {
    let enum_name = &def.enum_name;
    quote! {
        impl ::core::fmt::Display for #enum_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                write!(
                    f,
                    "{}",
                    <#enum_name as ::sbpf_common_tablegen::OpcodeTable>::to_str(self),
                )
            }
        }
    }
}

fn expand_try_from_u8(def: &OpcodeTableDef) -> TokenStream {
    let enum_name = &def.enum_name;
    let arms: Vec<_> = def
        .opcodes
        .iter()
        .filter(|op| !op.is_arch_v3())
        .map(|op| {
            let code = op.code;
            let variant = &op.variant;
            quote! { #code => Ok(#enum_name::#variant) }
        })
        .collect();

    quote! {
        impl ::core::convert::TryFrom<u8> for #enum_name {
            type Error = ::sbpf_common_tablegen::OpcodeError;

            fn try_from(opcode: u8) -> ::core::result::Result<Self, Self::Error> {
                match opcode {
                    #(#arms,)*
                    _ => Err(::sbpf_common_tablegen::OpcodeError::InvalidOpcode { byte: opcode }),
                }
            }
        }
    }
}

fn expand_from_opcode_for_u8(def: &OpcodeTableDef) -> TokenStream {
    let enum_name = &def.enum_name;
    let arms: Vec<_> = def
        .opcodes
        .iter()
        .map(|op| {
            let variant = &op.variant;
            let code = op.code;
            quote! { #enum_name::#variant => #code }
        })
        .collect();

    quote! {
        impl ::core::convert::From<#enum_name> for u8 {
            fn from(opcode: #enum_name) -> Self {
                match opcode {
                    #(#arms,)*
                }
            }
        }
    }
}

fn expand_opcode_table_trait(def: &OpcodeTableDef) -> TokenStream {
    let enum_name = &def.enum_name;
    let group_ty = group_enum_path(
        &def.opcodes
            .first()
            .expect("validated OpcodeTable must not be empty")
            .group,
    );

    let to_str_arms: Vec<_> = def
        .opcodes
        .iter()
        .map(|op| {
            let variant = &op.variant;
            let mnemonic = &op.mnemonic;
            quote! { #enum_name::#variant => #mnemonic }
        })
        .collect();

    let to_operator_arms: Vec<_> = def
        .opcodes
        .iter()
        .map(|op| {
            let variant = &op.variant;
            match &op.operator {
                Some(operator) => {
                    quote! { #enum_name::#variant => ::core::option::Option::Some(#operator) }
                }
                None => quote! { #enum_name::#variant => ::core::option::Option::None },
            }
        })
        .collect();

    let to_size_arms: Vec<_> = def
        .opcodes
        .iter()
        .map(|op| {
            let variant = &op.variant;
            match &op.size {
                Some(size) => {
                    quote! { #enum_name::#variant => ::core::option::Option::Some(#size) }
                }
                None => quote! { #enum_name::#variant => ::core::option::Option::None },
            }
        })
        .collect();

    let is_32bit_arms: Vec<_> = def
        .opcodes
        .iter()
        .map(|op| {
            let variant = &op.variant;
            let is_32 = op.variant_name().contains("32");
            quote! { #enum_name::#variant => #is_32 }
        })
        .collect();

    let v3_arms: Vec<_> = def
        .opcodes
        .iter()
        .filter(|op| op.is_arch_v3())
        .map(|op| {
            let code = op.code;
            let variant = &op.variant;
            quote! { #code => Ok(#enum_name::#variant) }
        })
        .collect();

    let group_arms: Vec<_> = def
        .opcodes
        .iter()
        .map(|op| {
            let variant = &op.variant;
            let group = &op.group;
            quote! { #enum_name::#variant => #group }
        })
        .collect();

    let all_variants: Vec<_> = def.opcodes.iter().map(|op| &op.variant).collect();

    let mut by_group_order: Vec<syn::Path> = Vec::new();
    let mut by_group_map: std::collections::HashMap<String, Vec<&syn::Ident>> =
        std::collections::HashMap::new();
    for op in &def.opcodes {
        let key = path_key(&op.group);
        if !by_group_map.contains_key(&key) {
            by_group_order.push(op.group.clone());
        }
        by_group_map.entry(key).or_default().push(&op.variant);
    }
    let by_group_arms: Vec<_> = by_group_order
        .iter()
        .map(|group_path| {
            let key = path_key(group_path);
            let variants = by_group_map.get(&key).expect("expected group key");
            quote! {
                #group_path => &[#(#enum_name::#variants),*]
            }
        })
        .collect();

    quote! {
        impl ::sbpf_common_tablegen::OpcodeTable for #enum_name {
            type Group = #group_ty;

            fn to_str(&self) -> &'static str {
                match self {
                    #(#to_str_arms,)*
                }
            }

            fn to_operator(&self) -> ::core::option::Option<&'static str> {
                match self {
                    #(#to_operator_arms,)*
                }
            }

            fn to_size(&self) -> ::core::option::Option<&'static str> {
                match self {
                    #(#to_size_arms,)*
                }
            }

            fn from_size(size: &str, group: Self::Group) -> ::core::option::Option<Self> {
                Self::by_group(group)
                    .iter()
                    .copied()
                    .find(|op| op.to_size() == ::core::option::Option::Some(size))
            }

            fn is_32bit(&self) -> bool {
                match self {
                    #(#is_32bit_arms,)*
                }
            }

            fn group(self) -> Self::Group {
                match self {
                    #(#group_arms,)*
                }
            }

            fn by_group(group: Self::Group) -> &'static [Self] {
                match group {
                    #(#by_group_arms,)*
                }
            }

            /// Decode opcode byte with sBPF v3 semantics.
            fn try_from_sbpf_v3(
                opcode: u8,
            ) -> ::core::result::Result<Self, ::sbpf_common_tablegen::OpcodeError> {
                match opcode {

                    #(#v3_arms,)*
                    _ => ::core::convert::TryInto::try_into(opcode),
                }
            }

            fn all() -> &'static [Self] {
                &[#(#enum_name::#all_variants),*]
            }
        }
    }
}

fn path_key(path: &Path) -> String {
    path.segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn group_enum_path(path: &Path) -> Path {
    let mut ty = path.clone();
    if ty.segments.len() > 1 {
        ty.segments.pop();
        let segs = ty.segments.into_iter().collect::<Vec<_>>();
        ty.segments = segs.into_iter().collect();
    }
    ty
}

/// Expand `#[derive(OpcodeGroup)]` into `HandlerTypes` and `OpcodeGroup` trait impls.
pub fn expand_opcode_group(def: &OpcodeGroupDef) -> TokenStream {
    let enum_name = &def.enum_name;
    let decode_type = &def.handlers.decode;
    let validate_type = &def.handlers.validate;
    let execute_type = &def.handlers.execute;

    let title_arms: Vec<_> = def
        .groups
        .iter()
        .map(|g| {
            let v = &g.variant;
            let title = &g.title;
            quote! { #enum_name::#v => #title }
        })
        .collect();

    let desc_arms: Vec<_> = def
        .groups
        .iter()
        .map(|g| {
            let v = &g.variant;
            let desc = &g.description;
            quote! { #enum_name::#v => #desc }
        })
        .collect();

    let decode_arms: Vec<_> = def
        .groups
        .iter()
        .map(|g| {
            let v = &g.variant;
            let path = &g.decode;
            quote! { #enum_name::#v => #path }
        })
        .collect();

    let validate_arms: Vec<_> = def
        .groups
        .iter()
        .map(|g| {
            let v = &g.variant;
            let path = &g.validate;
            quote! { #enum_name::#v => #path }
        })
        .collect();

    let execute_arms: Vec<_> = def
        .groups
        .iter()
        .map(|g| {
            let v = &g.variant;
            let path = &g.execute;
            quote! { #enum_name::#v => #path }
        })
        .collect();

    let all_variants: Vec<_> = def.groups.iter().map(|g| &g.variant).collect();

    quote! {
        impl ::sbpf_common_tablegen::HandlerTypes for #enum_name {
            type DecodeFn = #decode_type;
            type ValidateFn = #validate_type;
            type ExecuteFn = #execute_type;
        }

        impl ::sbpf_common_tablegen::OpcodeGroup for #enum_name {
            fn title(self) -> &'static str {
                match self {
                    #(#title_arms,)*
                }
            }

            fn description(self) -> &'static str {
                match self {
                    #(#desc_arms,)*
                }
            }

            fn decode_fn(self) -> Self::DecodeFn {
                match self {
                    #(#decode_arms,)*
                }
            }

            fn validate_fn(self) -> Self::ValidateFn {
                match self {
                    #(#validate_arms,)*
                }
            }

            fn execute_fn(self) -> Self::ExecuteFn {
                match self {
                    #(#execute_arms,)*
                }
            }

            fn all() -> &'static [Self] {
                &[#(#enum_name::#all_variants),*]
            }
        }
    }
}
