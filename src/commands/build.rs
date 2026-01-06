use {
    anyhow::{Error, Result},
    codespan_reporting::{
        diagnostic::{Diagnostic, Label},
        files::SimpleFile,
        term,
    },
    ed25519_dalek::SigningKey,
    rand::rngs::OsRng,
    sbpf_assembler::{assemble, assemble_with_debug_data, errors::CompileError},
    std::{fs, fs::create_dir_all, path::Path, time::Instant},
    termcolor::{ColorChoice, StandardStream},
};

pub trait AsDiagnostic {
    // currently only support single source file reporting
    fn to_diagnostic(&self) -> Diagnostic<()>;
}

impl AsDiagnostic for CompileError {
    fn to_diagnostic(&self) -> Diagnostic<()> {
        match self {
            // Show both the redefinition and the original definition
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

pub fn build(debug: bool, static_syscalls: bool) -> Result<()> {
    // Set src/out directory
    let src = "src";
    let deploy = "deploy";

    // Create necessary directories
    create_dir_all(deploy)?;

    // Function to compile assembly
    fn compile_assembly(src: &str, deploy: &str, debug: bool, static_syscalls: bool) -> Result<()> {
        let source_code = std::fs::read_to_string(src).unwrap();
        let file = SimpleFile::new(src.to_string(), source_code.clone());

        // Assemble with or without debug info
        let result = if debug {
            let filename = Path::new(src)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown.s");
            let directory = Path::new(src)
                .parent()
                .and_then(|p| p.canonicalize().ok())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string());
            assemble_with_debug_data(&source_code, filename, &directory, static_syscalls)
        } else {
            assemble(&source_code, static_syscalls)
        };

        let bytecode = match result {
            Ok(bytecode) => bytecode,
            Err(errors) => {
                for error in errors {
                    let writer = StandardStream::stderr(ColorChoice::Auto);
                    let config = term::Config::default();
                    let diagnostic = error.to_diagnostic();
                    term::emit(&mut writer.lock(), &config, &file, &diagnostic)?;
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
                    if debug { " (debug)" } else { "" }
                );
                let start = Instant::now();
                compile_assembly(&asm_file, deploy, debug, static_syscalls)?;
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
