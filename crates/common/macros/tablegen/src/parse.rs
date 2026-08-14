use {
    crate::{
        error::err,
        ir::{GroupDef, HandlerTypePaths, OpcodeArch, OpcodeDef, OpcodeGroupDef, OpcodeTableDef},
    },
    syn::{Attribute, Data, DeriveInput, Expr, Fields, Lit, Path, Variant, spanned::Spanned},
};

/// Parse an enum with `#[opcode(...)]` on each variant.
pub fn parse_opcode_table(input: &DeriveInput) -> syn::Result<OpcodeTableDef> {
    let enum_name = input.ident.clone();
    let data = match &input.data {
        Data::Enum(data) => data,
        _ => {
            return Err(err(
                enum_name.span(),
                "OpcodeTable can only be derived for enums",
            ));
        }
    };

    let mut opcodes = Vec::new();
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(err(
                variant.ident.span(),
                "OpcodeTable variants must be unit variants",
            ));
        }
        let attr = find_attr(&variant.attrs, "opcode")?.ok_or_else(|| {
            err(
                variant.ident.span(),
                format!("variant `{}` is missing #[opcode(...)]", variant.ident),
            )
        })?;
        opcodes.push(parse_opcode_attr(variant, attr)?);
    }

    Ok(OpcodeTableDef { enum_name, opcodes })
}

/// Parse an enum with `#[group(...)]` on each variant.
pub fn parse_opcode_group(input: &DeriveInput) -> syn::Result<OpcodeGroupDef> {
    let enum_name = input.ident.clone();
    let data = match &input.data {
        Data::Enum(data) => data,
        _ => {
            return Err(err(
                enum_name.span(),
                "OpcodeGroup can only be derived for enums",
            ));
        }
    };

    let handlers_attr = find_attr(&input.attrs, "handlers")?
        .ok_or_else(|| err(enum_name.span(), "OpcodeGroup requires #[handlers(...)]"))?;
    let handlers = parse_handlers_attr(handlers_attr)?;

    let mut groups = Vec::new();
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(err(
                variant.ident.span(),
                "OpcodeGroup variants must be unit variants",
            ));
        }
        let attr = find_attr(&variant.attrs, "group")?.ok_or_else(|| {
            err(
                variant.ident.span(),
                format!("variant `{}` is missing #[group(...)]", variant.ident),
            )
        })?;
        groups.push(parse_group_attr(variant, attr)?);
    }

    Ok(OpcodeGroupDef {
        enum_name,
        handlers,
        groups,
    })
}

fn find_attr<'a>(attrs: &'a [Attribute], name: &str) -> syn::Result<Option<&'a Attribute>> {
    let mut matches = attrs.iter().filter(|attr| attr.path().is_ident(name));
    let attr = matches.next();
    if let Some(duplicate) = matches.next() {
        return Err(err(
            duplicate.path().span(),
            format!("#[{name}(...)] specified more than once"),
        ));
    }
    Ok(attr)
}

fn parse_opcode_attr(variant: &Variant, attr: &Attribute) -> syn::Result<OpcodeDef> {
    let mut mnemonic: Option<String> = None;
    let mut code: Option<u8> = None;
    let mut group: Option<Path> = None;
    let mut doc: Option<String> = None;
    let mut operator: Option<String> = None;
    let mut size: Option<String> = None;
    let mut arch: Option<OpcodeArch> = None;

    attr.parse_nested_meta(|meta| {
        let key = meta
            .path
            .get_ident()
            .map(|i| i.to_string())
            .unwrap_or_default();

        match key.as_str() {
            "mnemonic" => {
                parse_once(&mut mnemonic, &meta, "mnemonic", parse_string_value)?;
            }
            "code" => {
                parse_once(&mut code, &meta, "code", parse_u8_value)?;
            }
            "group" => {
                parse_once(&mut group, &meta, "group", parse_path_value)?;
            }
            "doc" => {
                parse_once(&mut doc, &meta, "doc", parse_string_value)?;
            }
            "operator" => {
                parse_once(&mut operator, &meta, "operator", parse_string_value)?;
            }
            "size" => {
                parse_once(&mut size, &meta, "size", parse_string_value)?;
            }
            "arch" => {
                parse_once(&mut arch, &meta, "arch", parse_arch_value)?;
            }
            other => {
                return Err(meta.error(format!("unknown opcode field `{other}`")));
            }
        }
        Ok(())
    })?;

    let span = variant.ident.span();
    let mnemonic = mnemonic.ok_or_else(|| err(span, "missing required field `mnemonic`"))?;
    let code = code.ok_or_else(|| err(span, "missing required field `code`"))?;
    let group = group.ok_or_else(|| err(span, "missing required field `group`"))?;
    let doc = doc.ok_or_else(|| err(span, "missing required field `doc`"))?;

    Ok(OpcodeDef {
        variant: variant.ident.clone(),
        mnemonic,
        code,
        group,
        doc,
        operator,
        size,
        arch,
        span,
    })
}

fn parse_group_attr(variant: &Variant, attr: &Attribute) -> syn::Result<GroupDef> {
    let mut title: Option<String> = None;
    let mut description: Option<String> = None;
    let mut decode: Option<Path> = None;
    let mut validate: Option<Path> = None;
    let mut execute: Option<Path> = None;

    attr.parse_nested_meta(|meta| {
        let key = meta
            .path
            .get_ident()
            .map(|i| i.to_string())
            .unwrap_or_default();

        match key.as_str() {
            "title" => parse_once(&mut title, &meta, "title", parse_string_value)?,
            "description" => {
                parse_once(&mut description, &meta, "description", parse_string_value)?;
            }
            "decode" => parse_once(&mut decode, &meta, "decode", parse_path_value)?,
            "validate" => parse_once(&mut validate, &meta, "validate", parse_path_value)?,
            "execute" => parse_once(&mut execute, &meta, "execute", parse_path_value)?,
            other => {
                return Err(meta.error(format!("unknown group field `{other}`")));
            }
        }
        Ok(())
    })?;

    let span = variant.ident.span();
    let title = title.ok_or_else(|| err(span, "missing required field `title`"))?;
    let description =
        description.ok_or_else(|| err(span, "missing required field `description`"))?;
    let decode = decode.ok_or_else(|| err(span, "missing required field `decode`"))?;
    let validate = validate.ok_or_else(|| err(span, "missing required field `validate`"))?;
    let execute = execute.ok_or_else(|| err(span, "missing required field `execute`"))?;

    Ok(GroupDef {
        variant: variant.ident.clone(),
        title,
        description,
        decode,
        validate,
        execute,
        span,
    })
}

fn parse_handlers_attr(attr: &Attribute) -> syn::Result<HandlerTypePaths> {
    let mut decode: Option<Path> = None;
    let mut validate: Option<Path> = None;
    let mut execute: Option<Path> = None;

    attr.parse_nested_meta(|meta| {
        let key = meta
            .path
            .get_ident()
            .map(|i| i.to_string())
            .unwrap_or_default();

        match key.as_str() {
            "decode" => parse_once(&mut decode, &meta, "decode", parse_path_value)?,
            "validate" => parse_once(&mut validate, &meta, "validate", parse_path_value)?,
            "execute" => parse_once(&mut execute, &meta, "execute", parse_path_value)?,
            other => {
                return Err(meta.error(format!("unknown handler type field `{other}`")));
            }
        }
        Ok(())
    })?;

    let span = attr.path().span();
    let decode = decode.ok_or_else(|| err(span, "missing required handler type `decode`"))?;
    let validate = validate.ok_or_else(|| err(span, "missing required handler type `validate`"))?;
    let execute = execute.ok_or_else(|| err(span, "missing required handler type `execute`"))?;

    Ok(HandlerTypePaths {
        decode,
        validate,
        execute,
        span,
    })
}

fn parse_once<T>(
    slot: &mut Option<T>,
    meta: &syn::meta::ParseNestedMeta<'_>,
    field: &str,
    parse: impl FnOnce(&syn::meta::ParseNestedMeta<'_>) -> syn::Result<T>,
) -> syn::Result<()> {
    if slot.is_some() {
        return Err(meta.error(format!("`{field}` specified more than once")));
    }
    *slot = Some(parse(meta)?);
    Ok(())
}

fn parse_string_value(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<String> {
    let value = meta.value()?;
    let lit: Lit = value.parse()?;
    match lit {
        Lit::Str(s) => Ok(s.value()),
        _ => Err(meta.error("expected string literal")),
    }
}

fn parse_u8_value(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<u8> {
    let value = meta.value()?;
    let expr: Expr = value.parse()?;
    match &expr {
        Expr::Lit(syn::ExprLit {
            lit: Lit::Int(i), ..
        }) => {
            let n: u64 = i.base10_parse()?;
            if n > u8::MAX as u64 {
                return Err(meta.error("opcode code must fit in u8"));
            }
            Ok(n as u8)
        }
        _ => Err(meta.error("expected integer literal for `code`")),
    }
}

fn parse_path_value(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<Path> {
    let value = meta.value()?;
    let expr: Expr = value.parse()?;
    match expr {
        Expr::Path(p) => Ok(p.path),
        _ => Err(meta.error("expected path")),
    }
}

fn parse_arch_value(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<OpcodeArch> {
    let value = meta.value()?;
    let expr: Expr = value.parse()?;
    let name = match &expr {
        Expr::Path(p) if p.path.segments.len() == 1 => p.path.segments[0].ident.to_string(),
        _ => {
            return Err(meta.error("expected `arch = v2` or `arch = v3`"));
        }
    };
    match name.to_ascii_lowercase().as_str() {
        "v2" => Ok(OpcodeArch::V2),
        "v3" => Ok(OpcodeArch::V3),
        _ => Err(meta.error(format!("unknown arch `{name}`; expected v2 or v3"))),
    }
}
