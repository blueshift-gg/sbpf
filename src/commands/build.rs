use {
    anyhow::{Error, Result},
    clap::{Args, ValueEnum},
    codespan_reporting::{
        diagnostic::{Diagnostic, Label},
        files::SimpleFiles,
        term,
    },
    ed25519_dalek::SigningKey,
    rand::rngs::OsRng,
    sbpf_assembler::{
        AssembleErrors, Assembler, AssemblerOption, DebugMode, FileRegistry, FsFileResolver,
        SbpfArch, SourceOrigin, errors::CompileError,
    },
    std::{collections::HashMap, fs, fs::create_dir_all, path::Path, time::Instant},
    termcolor::{ColorChoice, StandardStream},
};

#[derive(Args, Default)]
pub struct BuildArgs {
    #[arg(short = 'g', long, help = "Include debug information")]
    pub debug: bool,
    #[arg(
        short = 'a',
        long,
        default_value = "v0",
        help = "Target architecture (v0 or v3)"
    )]
    arch: ArchArg,
    #[arg(short = 'd', long, help = "Output deploy directory")]
    pub deploy_dir: Option<String>,
}

#[derive(Clone, Copy, ValueEnum, Default)]
pub enum ArchArg {
    #[default]
    V0,
    V3,
}

impl From<ArchArg> for SbpfArch {
    fn from(arg: ArchArg) -> Self {
        match arg {
            ArchArg::V0 => SbpfArch::V0,
            ArchArg::V3 => SbpfArch::V3,
        }
    }
}

pub trait AsDiagnostic<FileId> {
    fn to_diagnostic(&self) -> Diagnostic<FileId>;
}

impl AsDiagnostic<()> for CompileError {
    fn to_diagnostic(&self) -> Diagnostic<()> {
        match self {
            CompileError::DuplicateLabel {
                span,
                original_span,
                ..
            } => Diagnostic::error()
                .with_message(self.to_string())
                .with_labels(vec![
                    Label::primary((), span.start..span.end).with_message(self.label()),
                    Label::secondary((), original_span.start..original_span.end)
                        .with_message("previous definition is here"),
                ]),
            _ => Diagnostic::error()
                .with_message(self.to_string())
                .with_labels(vec![
                    Label::primary((), self.span().start..self.span().end)
                        .with_message(self.label()),
                ]),
        }
    }
}

/// Render assembly errors against original source files using the FileRegistry.
///
/// Each error's `SourceOrigin` tells us which original file and line the error
/// came from, even if it was in a macro expansion or an included file.
fn emit_assembler_errors(assemble_errors: &AssembleErrors) -> Result<()> {
    let registry = &assemble_errors.file_registry;

    // Build a codespan SimpleFiles from the FileRegistry
    let mut files = SimpleFiles::new();
    let mut file_id_map: HashMap<u32, usize> = HashMap::new();

    for file_id in registry.file_ids() {
        let cs_id = files.add(registry.path(file_id).to_string(), registry.content(file_id).to_string());
        file_id_map.insert(file_id.index(), cs_id);
    }

    let writer = StandardStream::stderr(ColorChoice::Auto);
    let config = term::Config::default();

    for assembler_error in &assemble_errors.errors {
        let error = &assembler_error.error;

        if let Some(ref origin) = assembler_error.origin {
            // We have a resolved source origin -- point into the original file
            if let Some(&cs_file_id) = file_id_map.get(&(origin.file_id.index())) {
                let line_start = registry.line_byte_offset(origin.file_id, origin.line);
                let line_len = registry.line_length(origin.file_id, origin.line);
                let line_end = line_start + line_len;

                // If we have column info, highlight from that column to end of line.
                // Otherwise highlight the whole line.
                let highlight_start = if let Some(col) = assembler_error.column {
                    (line_start + col).min(line_end)
                } else {
                    line_start
                };

                let mut diagnostic = Diagnostic::error()
                    .with_message(error.to_string())
                    .with_labels(vec![
                        Label::primary(cs_file_id, highlight_start..line_end)
                            .with_message(error.label()),
                    ]);

                // Add macro expansion chain as notes
                let mut notes = Vec::new();
                build_expansion_notes(origin, registry, &mut notes);
                if !notes.is_empty() {
                    diagnostic = diagnostic.with_notes(notes);
                }

                term::emit(&mut writer.lock(), &config, &files, &diagnostic)?;
            } else {
                // File not in registry (shouldn't happen), fall back to text-only
                eprintln!("error: {}", error);
            }
        } else {
            // No origin -- preprocessor error without file context, just print the message
            eprintln!("error: {}", error);
        }
    }

    Ok(())
}

/// Build notes describing the macro expansion chain for an error.
fn build_expansion_notes(
    origin: &SourceOrigin,
    registry: &FileRegistry,
    notes: &mut Vec<String>,
) {
    if let Some(ref expansion) = origin.macro_expansion {
        // Note about which macro this is in
        let invocation = &expansion.invocation_origin;
        let inv_file = registry.path(invocation.file_id);
        notes.push(format!(
            "in expansion of macro '{}', invoked at {}:{}",
            expansion.macro_name, inv_file, invocation.line
        ));

        // Recurse for nested expansions
        if invocation.macro_expansion.is_some() {
            build_expansion_notes(invocation, registry, notes);
        }
    }
}

pub fn build(args: BuildArgs) -> Result<()> {
    // Set src/out directory
    let src = "src";
    let deploy = args.deploy_dir.as_deref().unwrap_or("deploy");

    // Create necessary directories
    create_dir_all(deploy)?;
    // Function to compile assembly with preprocessing (includes + macros)
    fn compile_assembly(src: &str, deploy: &str, debug: bool, arch: SbpfArch) -> Result<()> {
        let source_code = std::fs::read_to_string(src)
            .map_err(|e| Error::msg(format!("Failed to read '{}': {}", src, e)))?;

        // Build assembler options
        let debug_mode = if debug {
            let filename = Path::new(src)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown.s");
            let directory = Path::new(src)
                .parent()
                .and_then(|p| p.canonicalize().ok())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string());
            Some(DebugMode {
                filename: filename.to_string(),
                directory,
            })
        } else {
            None
        };

        let options = AssemblerOption { arch, debug_mode };
        let assembler = Assembler::new(options);
        let resolver = FsFileResolver::new();

        let result = assembler.assemble_with_preprocess(&source_code, src, Some(&resolver));

        let bytecode = match result {
            Ok(bytecode) => bytecode,
            Err(assemble_errors) => {
                emit_assembler_errors(&assemble_errors)?;
                return Err(Error::msg("Compilation failed"));
            }
        };

        // write bytecode to <filename>.so
        let output_path = Path::new(deploy).join(
            Path::new(src)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .replace(".s", ".so"),
        );

        std::fs::write(output_path, bytecode)?;
        Ok(())
    }

    // Function to check if keypair file exists.
    fn has_keypair_file(dir: &Path) -> bool {
        if dir.exists() && dir.is_dir() {
            match fs::read_dir(dir) {
                Ok(entries) => entries.filter_map(Result::ok).any(|entry| {
                    entry
                        .path()
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.ends_with("-keypair.json"))
                        .unwrap_or(false)
                }),
                Err(_) => false,
            }
        } else {
            false
        }
    }

    // Check if keypair file exists. If not, create one.
    let deploy_path = Path::new(deploy);
    if !has_keypair_file(deploy_path) {
        let project_path = std::env::current_dir()?;
        let project_name = project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("program");
        let mut rng = OsRng;
        fs::write(
            deploy_path.join(format!("{}-keypair.json", project_name)),
            serde_json::json!(SigningKey::generate(&mut rng).to_keypair_bytes()[..]).to_string(),
        )?;
    }

    // Processing directories
    let src_path = Path::new(src);
    for entry in src_path.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir()
            && let Some(subdir) = path.file_name().and_then(|name| name.to_str())
        {
            let asm_file = format!("{}/{}/{}.s", src, subdir, subdir);
            if Path::new(&asm_file).exists() {
                println!(
                    "⚡️ Building \"{}\"{}",
                    subdir,
                    if args.debug { " (debug)" } else { "" }
                );
                let start = Instant::now();
                compile_assembly(&asm_file, deploy, args.debug, args.arch.into())?;
                let duration = start.elapsed();
                println!(
                    "✅ \"{}\" built successfully in {}ms!",
                    subdir,
                    duration.as_micros() as f64 / 1000.0
                );
            }
        }
    }

    Ok(())
}
