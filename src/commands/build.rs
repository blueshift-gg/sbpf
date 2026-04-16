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
    sbpf_assembler::{Assembler, AssemblerOption, DebugMode, SbpfArch, errors::CompileError},
    std::{
        collections::HashMap,
        fs::{self, create_dir_all},
        path::Path,
        time::Instant,
    },
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

/// Convert a [`CompileError`] into a [`codespan_reporting`] diagnostic.
///
/// The `resolve` closure maps a file identifier (as used by the parser —
/// either the main file name or a relative `.include` path) to a
/// [`SimpleFiles`] file id. For errors without explicit file info the
/// `default_file_id` is used.
///
/// Multi-file support: for [`CompileError::DuplicateLabel`] produced by
/// a multi-file parse (`assemble_with_base_path`), the error carries the
/// file for both the original and the duplicate definition, plus the
/// chain of `.include` directives that led to the duplicate. Each of
/// those becomes a label in the emitted diagnostic so the user sees:
///
/// * primary — the redefinition, in its own file
/// * secondary — the original definition, in its own file
/// * secondary — one per `.include` site in the chain ("included from
///   here"), annotating every `.include` directive that pulled in the
///   file containing the duplicate
pub trait AsDiagnostic {
    fn to_diagnostic<F>(&self, default_file_id: usize, resolve: F) -> Diagnostic<usize>
    where
        F: Fn(&str) -> usize;
}

impl AsDiagnostic for CompileError {
    fn to_diagnostic<F>(&self, default_file_id: usize, resolve: F) -> Diagnostic<usize>
    where
        F: Fn(&str) -> usize,
    {
        match self {
            CompileError::DuplicateLabel {
                span,
                original_span,
                multi,
                ..
            } => {
                // Resolve the file ids for the two definitions. In
                // single-file mode (no `.include`), both fall back to
                // `default_file_id`. In multi-file mode, the parser
                // populates `multi` with the actual file names.
                let (span_id, original_id) = if let Some(m) = multi.as_deref() {
                    (resolve(&m.span_file), resolve(&m.original_span_file))
                } else {
                    (default_file_id, default_file_id)
                };
                let mut labels = vec![
                    Label::primary(span_id, span.start..span.end).with_message(self.label()),
                    Label::secondary(original_id, original_span.start..original_span.end)
                        .with_message("previous definition is here"),
                ];
                // When the duplicate originates inside an included file,
                // annotate every `.include` directive in the chain so
                // the user can trace where the second definition came
                // from.
                if let Some(m) = multi.as_deref() {
                    for (include_file, include_span) in &m.include_chain {
                        let file_id = resolve(include_file);
                        labels.push(
                            Label::secondary(file_id, include_span.start..include_span.end)
                                .with_message("included from here"),
                        );
                    }
                }
                Diagnostic::error()
                    .with_message(self.to_string())
                    .with_labels(labels)
            }
            _ => {
                let file_id = match self.file() {
                    Some(f) => resolve(f),
                    None => default_file_id,
                };
                Diagnostic::error()
                    .with_message(self.to_string())
                    .with_labels(vec![
                        Label::primary(file_id, self.span().start..self.span().end)
                            .with_message(self.label()),
                    ])
            }
        }
    }
}

pub fn build(args: BuildArgs) -> Result<()> {
    // Set src/out directory
    let src = "src";
    let deploy = args.deploy_dir.as_deref().unwrap_or("deploy");

    // Create necessary directories
    create_dir_all(deploy)?;
    // Function to compile assembly
    fn compile_assembly(src: &str, deploy: &str, debug: bool, arch: SbpfArch) -> Result<()> {
        let source_code = std::fs::read_to_string(src).unwrap();

        let main_file_name = Path::new(src)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.s")
            .to_string();
        let base_path_buf = {
            let p = Path::new(src)
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| Path::new(".").to_path_buf());
            p.canonicalize().unwrap_or(p)
        };

        // Build assembler options
        let debug_mode = if debug {
            let directory = base_path_buf.to_string_lossy().to_string();
            Some(DebugMode {
                filename: main_file_name.clone(),
                directory,
            })
        } else {
            None
        };

        let options = AssemblerOption { arch, debug_mode };

        let assembler = Assembler::new(options);
        let result = assembler.assemble_with_base_path(
            &main_file_name,
            &source_code,
            base_path_buf.as_path(),
        );

        let bytecode = match result.bytecode {
            Ok(bytecode) => bytecode,
            Err(errors) => {
                // Build a SimpleFiles registry only when there is an
                // error — this is the only code path that needs it.
                let mut files = SimpleFiles::new();
                let mut file_ids: HashMap<String, usize> = HashMap::new();
                let main_content = result
                    .sources
                    .get(&main_file_name)
                    .cloned()
                    .unwrap_or_else(|| source_code.clone());
                let main_file_id = files.add(main_file_name.clone(), main_content);
                file_ids.insert(main_file_name.clone(), main_file_id);
                for (name, content) in &result.sources {
                    if name == &main_file_name {
                        continue;
                    }
                    let id = files.add(name.clone(), content.clone());
                    file_ids.insert(name.clone(), id);
                }
                let resolve =
                    |file: &str| -> usize { *file_ids.get(file).unwrap_or(&main_file_id) };

                for error in errors {
                    let writer = StandardStream::stderr(ColorChoice::Auto);
                    let config = term::Config::default();
                    let diagnostic = error.to_diagnostic(main_file_id, resolve);
                    term::emit(&mut writer.lock(), &config, &files, &diagnostic)?;
                }
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
