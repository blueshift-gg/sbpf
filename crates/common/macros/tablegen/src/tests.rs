#[cfg(test)]
mod tests {
    use {
        crate::{
            expand::{expand_opcode_group, expand_opcode_table},
            parse::{parse_opcode_group, parse_opcode_table},
            validate::{validate_opcode_group, validate_opcode_table},
        },
        syn::{DeriveInput, parse_quote},
    };

    fn test_opcode_table_input() -> DeriveInput {
        parse_quote! {
            pub enum O {
                #[opcode(
                    mnemonic = "op1",
                    code = 0x01,
                    group = G::A,
                    doc = "op1 imm",
                    operator = "+=",
                )]
                Op1Imm,

                #[opcode(
                    mnemonic = "op1",
                    code = 0x02,
                    group = G::B,
                    doc = "op1 reg",
                    operator = "+=",
                )]
                Op1Reg,

                #[opcode(
                    mnemonic = "op2",
                    code = 0x03,
                    group = G::C,
                    doc = "op2",
                    arch = v3,
                )]
                Op2,

                #[opcode(
                    mnemonic = "op3",
                    code = 0x04,
                    group = G::D,
                    doc = "op3",
                )]
                Op3,
            }
        }
    }

    fn test_opcode_group_input() -> DeriveInput {
        parse_quote! {
            #[handlers(
                decode = DecodeFn,
                validate = ValidateFn,
                execute = ExecuteFn,
            )]
            pub enum G {
                #[group(
                    title = "A",
                    description = "Group A",
                    decode = dec_a,
                    validate = val_a,
                    execute = exe_a,
                )]
                A,

                #[group(
                    title = "B",
                    description = "Group B",
                    decode = dec_b,
                    validate = val_b,
                    execute = exe_b,
                )]
                B,

                #[group(
                    title = "C",
                    description = "Group C",
                    decode = dec_c,
                    validate = val_c,
                    execute = exe_c,
                )]
                C,

                #[group(
                    title = "D",
                    description = "Group D",
                    decode = dec_d,
                    validate = val_d,
                    execute = exe_d,
                )]
                D,
            }
        }
    }

    // Parse tests

    #[test]
    fn parse_opcode_table_fields() {
        let opcode_table_def = parse_opcode_table(&test_opcode_table_input()).unwrap();
        assert_eq!(opcode_table_def.enum_name, "O");
        assert_eq!(opcode_table_def.opcodes.len(), 4);

        let op1_imm = &opcode_table_def.opcodes[0];
        assert_eq!(op1_imm.variant_name(), "Op1Imm");
        assert_eq!(op1_imm.mnemonic, "op1");
        assert_eq!(op1_imm.code, 0x01);
        assert_eq!(op1_imm.operator.as_deref(), Some("+="));
        assert!(op1_imm.arch.is_none());
        assert!(!op1_imm.is_arch_v3());
        assert_eq!(op1_imm.group_variant_name().as_deref(), Some("A"));

        let op2 = opcode_table_def
            .opcodes
            .iter()
            .find(|o| o.variant_name() == "Op2")
            .unwrap();
        assert!(op2.is_arch_v3());
        assert_eq!(op2.code, 0x03);
    }

    #[test]
    fn parse_opcode_group_fields() {
        let opcode_group_def = parse_opcode_group(&test_opcode_group_input()).unwrap();
        assert_eq!(opcode_group_def.groups.len(), 4);
        assert_eq!(opcode_group_def.groups[0].title, "A");
        assert_eq!(
            opcode_group_def
                .handlers
                .decode
                .segments
                .last()
                .unwrap()
                .ident,
            "DecodeFn"
        );
        assert_eq!(
            opcode_group_def.groups[0]
                .decode
                .segments
                .last()
                .unwrap()
                .ident
                .to_string(),
            "dec_a"
        );
    }

    #[test]
    fn missing_group_handler_path_errors() {
        let input: DeriveInput = parse_quote! {
            #[handlers(
                decode = DecodeFn,
                validate = ValidateFn,
                execute = ExecuteFn,
            )]
            pub enum G {
                #[group(title = "A", description = "Group A")]
                A,
            }
        };
        let err = parse_opcode_group(&input).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("decode") || msg.contains("validate") || msg.contains("execute"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn missing_handlers_errors() {
        let input: DeriveInput = parse_quote! {
            pub enum G {
                #[group(
                    title = "A",
                    description = "Group A",
                    decode = dec_a,
                    validate = val_a,
                    execute = exe_a,
                )]
                A,
            }
        };
        let err = parse_opcode_group(&input).unwrap_err();
        assert!(
            err.to_string().contains("handlers"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn missing_opcode_attr_errors() {
        let input: DeriveInput = parse_quote! {
            pub enum Opcode {
                Bare,
            }
        };
        let err = parse_opcode_table(&input).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("missing") && msg.contains("opcode"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn missing_required_field_errors() {
        let input: DeriveInput = parse_quote! {
            pub enum Opcode {
                #[opcode(mnemonic = "x", code = 0x01, group = G::A)]
                X,
            }
        };
        let err = parse_opcode_table(&input).unwrap_err();
        assert!(
            err.to_string().contains("doc"),
            "expected missing doc error, got {}",
            err
        );
    }

    #[test]
    fn duplicate_opcode_field_errors() {
        let input: DeriveInput = parse_quote! {
            pub enum Opcode {
                #[opcode(
                    mnemonic = "x",
                    code = 0x01,
                    code = 0x02,
                    group = G::A,
                    doc = "x",
                )]
                X,
            }
        };
        let err = parse_opcode_table(&input).unwrap_err();
        assert_eq!(err.to_string(), "`code` specified more than once");
    }

    #[test]
    fn duplicate_group_field_errors() {
        let input: DeriveInput = parse_quote! {
            #[handlers(decode = DecodeFn, validate = ValidateFn, execute = ExecuteFn)]
            pub enum G {
                #[group(
                    title = "A",
                    title = "Duplicate A",
                    description = "Group A",
                    decode = dec_a,
                    validate = val_a,
                    execute = exe_a,
                )]
                A,
            }
        };
        let err = parse_opcode_group(&input).unwrap_err();
        assert_eq!(err.to_string(), "`title` specified more than once");
    }

    #[test]
    fn duplicate_handlers_field_errors() {
        let input: DeriveInput = parse_quote! {
            #[handlers(
                decode = DecodeFn,
                decode = DecodeFn2,
                validate = ValidateFn,
                execute = ExecuteFn,
            )]
            pub enum G {
                #[group(
                    title = "A",
                    description = "Group A",
                    decode = dec_a,
                    validate = val_a,
                    execute = exe_a,
                )]
                A,
            }
        };
        let err = parse_opcode_group(&input).unwrap_err();
        assert_eq!(err.to_string(), "`decode` specified more than once");
    }

    #[test]
    fn duplicate_opcode_attrs_error() {
        let input: DeriveInput = parse_quote! {
            pub enum Opcode {
                #[opcode(mnemonic = "x", code = 0x01, group = G::A, doc = "x")]
                #[opcode(mnemonic = "y", code = 0x02, group = G::A, doc = "y")]
                X,
            }
        };
        let err = parse_opcode_table(&input).unwrap_err();
        assert_eq!(err.to_string(), "#[opcode(...)] specified more than once");
    }

    #[test]
    fn duplicate_group_attrs_error() {
        let input: DeriveInput = parse_quote! {
            #[handlers(decode = DecodeFn, validate = ValidateFn, execute = ExecuteFn)]
            pub enum G {
                #[group(
                    title = "A",
                    description = "Group A",
                    decode = dec_a,
                    validate = val_a,
                    execute = exe_a,
                )]
                #[group(
                    title = "Duplicate A",
                    description = "Duplicate group A",
                    decode = dec_a,
                    validate = val_a,
                    execute = exe_a,
                )]
                A,
            }
        };
        let err = parse_opcode_group(&input).unwrap_err();
        assert_eq!(err.to_string(), "#[group(...)] specified more than once");
    }

    #[test]
    fn duplicate_handlers_attrs_error() {
        let input: DeriveInput = parse_quote! {
            #[handlers(decode = DecodeFn, validate = ValidateFn, execute = ExecuteFn)]
            #[handlers(decode = DecodeFn, validate = ValidateFn, execute = ExecuteFn)]
            pub enum G {
                #[group(
                    title = "A",
                    description = "Group A",
                    decode = dec_a,
                    validate = val_a,
                    execute = exe_a,
                )]
                A,
            }
        };
        let err = parse_opcode_group(&input).unwrap_err();
        assert_eq!(err.to_string(), "#[handlers(...)] specified more than once");
    }

    // Validate tests.

    #[test]
    fn validate_opcode_table_ok() {
        let opcode_table_def = parse_opcode_table(&test_opcode_table_input()).unwrap();
        validate_opcode_table(&opcode_table_def).unwrap();
    }

    #[test]
    fn validate_opcode_group_ok() {
        let opcode_group_def = parse_opcode_group(&test_opcode_group_input()).unwrap();
        validate_opcode_group(&opcode_group_def).unwrap();
    }

    #[test]
    fn validate_rejects_duplicate_non_v3_code() {
        let input: DeriveInput = parse_quote! {
            pub enum Opcode {
                #[opcode(mnemonic = "a", code = 0x01, group = G::A, doc = "a")]
                A,
                #[opcode(mnemonic = "b", code = 0x01, group = G::B, doc = "b")]
                B,
            }
        };
        let opcode_table_def = parse_opcode_table(&input).unwrap();
        let err = validate_opcode_table(&opcode_table_def).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("duplicate non-v3") || msg.contains("0x01"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn validate_rejects_duplicate_v3_code() {
        let input: DeriveInput = parse_quote! {
            pub enum Opcode {
                #[opcode(mnemonic = "a", code = 0x01, group = G::A, doc = "a", arch = v3)]
                A,
                #[opcode(mnemonic = "b", code = 0x01, group = G::B, doc = "b", arch = v3)]
                B,
            }
        };
        let opcode_table_def = parse_opcode_table(&input).unwrap();
        let err = validate_opcode_table(&opcode_table_def).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("duplicate v3") || msg.contains("0x01"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn validate_rejects_empty_opcode_table() {
        let input: DeriveInput = parse_quote! {
            pub enum Opcode {}
        };
        let opcode_table_def = parse_opcode_table(&input).unwrap();
        let err = validate_opcode_table(&opcode_table_def).unwrap_err();
        assert!(
            err.to_string().contains("at least one opcode variant"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_rejects_empty_opcode_group() {
        let input: DeriveInput = parse_quote! {
            #[handlers(decode = DecodeFn, validate = ValidateFn, execute = ExecuteFn)]
            pub enum Group {}
        };
        let opcode_group_def = parse_opcode_group(&input).unwrap();
        let err = validate_opcode_group(&opcode_group_def).unwrap_err();
        assert!(
            err.to_string().contains("at least one group variant"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_allows_v3_and_non_v3_same_code() {
        let input: DeriveInput = parse_quote! {
            pub enum Opcode {
                #[opcode(mnemonic = "x", code = 0x01, group = G::A, doc = "non-v3")]
                X,
                #[opcode(
                    mnemonic = "y",
                    code = 0x01,
                    group = G::B,
                    doc = "v3-only",
                    arch = v3,
                )]
                Y,
            }
        };
        let opcode_table_def = parse_opcode_table(&input).unwrap();
        validate_opcode_table(&opcode_table_def).unwrap();
    }

    // Expand tests.

    #[test]
    fn expand_opcode_table_core_impls() {
        let opcode_table_def = parse_opcode_table(&test_opcode_table_input()).unwrap();
        validate_opcode_table(&opcode_table_def).unwrap();
        let s = expand_opcode_table(&opcode_table_def).to_string();

        assert!(s.contains("OpcodeTable for O"));
        assert!(s.contains("type Group = G"));
        assert!(s.contains("fn to_str"));
        assert!(s.contains("fn group"));
        assert!(s.contains("fn try_from_sbpf_v3"));
        assert!(s.contains("fn all"));
    }

    #[test]
    fn expand_opcode_group_core_impls() {
        let opcode_group_def = parse_opcode_group(&test_opcode_group_input()).unwrap();
        let s = expand_opcode_group(&opcode_group_def).to_string();

        assert!(s.contains("HandlerTypes for G"));
        assert!(s.contains("OpcodeGroup for G"));
        assert!(s.contains("fn title"));
        assert!(s.contains("fn description"));
        assert!(s.contains("fn decode_fn"));
        assert!(s.contains("fn validate_fn"));
        assert!(s.contains("fn execute_fn"));
        assert!(s.contains("fn all"));
    }
}
