use {
    anyhow::{Context, bail},
    quote::ToTokens,
    sbpf_common_tablegen::{OpcodeArch, parse_opcode_group, parse_opcode_table},
    std::{env, fmt::Write, fs, path::PathBuf},
    syn::{DeriveInput, Item},
};

const OPCODE_SRC: &str = "crates/common/src/opcode.rs";
const OPTYPE_SRC: &str = "crates/common/src/optype.rs";
const DOCS_PATH: &str = "docs/opcodes.md";

fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("gen-opcode-docs") => gen_opcode_docs(),
        _ => {
            bail!("unknown command");
        }
    }
}

fn gen_opcode_docs() -> anyhow::Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("failed to get root folder")
        .to_path_buf();
    let dest = root.join(DOCS_PATH);

    // Parse tablegen macros.
    let opcode_src = fs::read_to_string(root.join(OPCODE_SRC))
        .with_context(|| format!("failed to read {OPCODE_SRC}"))?;
    let optype_src = fs::read_to_string(root.join(OPTYPE_SRC))
        .with_context(|| format!("failed to read {OPTYPE_SRC}"))?;

    let opcode_file = syn::parse_file(&opcode_src)
        .map_err(|err| anyhow::anyhow!("failed to parse {OPCODE_SRC}: {err}"))?;
    let optype_file = syn::parse_file(&optype_src)
        .map_err(|err| anyhow::anyhow!("failed to parse {OPTYPE_SRC}: {err}"))?;

    let opcode_input = get_enum(&opcode_file, "Opcode")?;
    let optype_input = get_enum(&optype_file, "OperationType")?;

    let table = parse_opcode_table(&opcode_input).map_err(|err| anyhow::anyhow!("{err}"))?;
    let groups = parse_opcode_group(&optype_input).map_err(|err| anyhow::anyhow!("{err}"))?;

    // Create markdown document.
    let mut out = String::new();
    out.push_str("<!-- This document is auto-generated. Do not edit manually. -->\n");
    out.push('\n');
    out.push_str("# sBPF Opcode Reference\n");

    for group in &groups.groups {
        let name = group.variant_name();
        out.push('\n');
        out.push_str("## ");
        out.push_str(&group.title);
        out.push('\n');
        out.push('\n');
        out.push_str(&group.description);
        out.push('\n');
        out.push('\n');
        out.push_str("| Mnemonic | Opcode |  Usage  | Note |\n");
        out.push_str("|----------|--------|---------|------|\n");

        for op in table
            .opcodes
            .iter()
            .filter(|op| op.group_variant_name().as_deref() == Some(name.as_str()))
        {
            let note = match op.arch {
                Some(OpcodeArch::V2) => "v2 only",
                Some(OpcodeArch::V3) => "v3 only",
                None => "-",
            };
            let _ = writeln!(
                out,
                "| {} | `0x{:02x}` | `{}` | {} |",
                op.mnemonic, op.code, op.doc, note,
            );
        }
    }

    // Write document to file.
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&dest, out).with_context(|| format!("failed to write {}", dest.display()))?;
    println!("Generated opcode docs at: {}.", dest.display());
    Ok(())
}

fn get_enum(file: &syn::File, name: &str) -> anyhow::Result<DeriveInput> {
    for item in &file.items {
        if let Item::Enum(en) = item
            && en.ident == name
        {
            return syn::parse2(en.to_token_stream()).map_err(|err| anyhow::anyhow!("{err}"));
        }
    }
    bail!("enum `{name}` not found");
}
