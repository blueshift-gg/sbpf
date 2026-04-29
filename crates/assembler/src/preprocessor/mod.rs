pub mod expand;
pub mod include;
pub mod macro_def;
pub mod source_map;

use {
    crate::errors::CompileError,
    source_map::{FileRegistry, SourceMap, SourceOrigin},
    std::path::PathBuf,
};

/// A line of source with its origin tracking
#[derive(Debug, Clone)]
pub(crate) struct SourceLine {
    pub text: String,
    pub origin: SourceOrigin,
}

/// Trait for resolving `.include` file paths to their contents.
/// Abstracted as a trait so tests can use an in-memory mock.
pub trait FileResolver {
    /// Resolve an include path relative to the including file's directory.
    /// Returns the file contents.
    fn resolve(&self, path: &str, relative_to: &str) -> Result<String, std::io::Error>;
}

/// File resolver that reads from the real filesystem.
#[derive(Debug, Clone)]
pub struct FsFileResolver {
    /// Additional directories to search for includes
    pub include_paths: Vec<PathBuf>,
}

impl FsFileResolver {
    pub fn new() -> Self {
        Self {
            include_paths: Vec::new(),
        }
    }

    pub fn with_include_paths(include_paths: Vec<PathBuf>) -> Self {
        Self { include_paths }
    }
}

impl Default for FsFileResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl FileResolver for FsFileResolver {
    fn resolve(&self, path: &str, relative_to: &str) -> Result<String, std::io::Error> {
        // Try relative to the including file's directory first
        let base_dir = std::path::Path::new(relative_to)
            .parent()
            .unwrap_or(std::path::Path::new("."));
        let candidate = base_dir.join(path);
        if candidate.exists() {
            return std::fs::read_to_string(&candidate);
        }

        // Try each include path
        for include_dir in &self.include_paths {
            let candidate = include_dir.join(path);
            if candidate.exists() {
                return std::fs::read_to_string(&candidate);
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("file not found: {}", path),
        ))
    }
}

/// In-memory file resolver for testing
#[derive(Debug, Clone, Default)]
pub struct MockFileResolver {
    files: std::collections::HashMap<String, String>,
}

impl MockFileResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_file(&mut self, path: &str, content: &str) -> &mut Self {
        self.files.insert(path.to_string(), content.to_string());
        self
    }
}

impl FileResolver for MockFileResolver {
    fn resolve(&self, path: &str, _relative_to: &str) -> Result<String, std::io::Error> {
        self.files.get(path).cloned().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("file not found: {}", path),
            )
        })
    }
}

/// Result of preprocessing: expanded source text + a source map for diagnostics
pub struct PreprocessResult {
    pub expanded_source: String,
    pub source_map: SourceMap,
}

/// A preprocessor error paired with its source origin.
#[derive(Debug)]
pub struct PreprocessorError {
    pub error: CompileError,
    pub origin: Option<SourceOrigin>,
}

/// Returned on preprocessing failure: errors + the file registry
/// (so the caller can still render diagnostics against original files).
pub struct PreprocessFailure {
    pub errors: Vec<PreprocessorError>,
    pub file_registry: FileRegistry,
}

/// Run the full preprocessor pipeline:
/// 1. Resolve `.include` directives (flatten files)
/// 2. Expand `.macro`/`.endm`, `.rept`/`.endr`, `.irp`/`.endr`
///
/// The resulting `expanded_source` can be fed directly to the pest parser.
/// The `source_map` allows remapping pest error spans back to original locations.
pub fn preprocess(
    source: &str,
    source_path: &str,
    resolver: Option<&dyn FileResolver>,
) -> Result<PreprocessResult, PreprocessFailure> {
    let mut registry = FileRegistry::new();

    // Pass 1: Include resolution
    let lines = match include::resolve_includes(source, source_path, resolver, &mut registry) {
        Ok(lines) => lines,
        Err(errors) => {
            return Err(PreprocessFailure {
                errors: errors
                    .into_iter()
                    .map(|e| PreprocessorError {
                        error: e,
                        origin: None,
                    })
                    .collect(),
                file_registry: registry,
            });
        }
    };

    // Pass 2: Macro expansion
    let (expanded_lines, errors) = match expand::expand_macros(lines) {
        Ok(result) => result,
        Err(errors) => {
            return Err(PreprocessFailure {
                errors: errors
                    .into_iter()
                    .map(|e| PreprocessorError {
                        error: e.error,
                        origin: e.origin,
                    })
                    .collect(),
                file_registry: registry,
            });
        }
    };

    if !errors.is_empty() {
        return Err(PreprocessFailure {
            errors: errors
                .into_iter()
                .map(|e| PreprocessorError {
                    error: e.error,
                    origin: e.origin,
                })
                .collect(),
            file_registry: registry,
        });
    }

    // Build the expanded source string and source map
    let mut expanded_source = String::new();
    let mut line_origins = Vec::with_capacity(expanded_lines.len());

    for line in &expanded_lines {
        expanded_source.push_str(&line.text);
        expanded_source.push('\n');
        line_origins.push(line.origin.clone());
    }

    let source_map = SourceMap::new(registry, line_origins);

    Ok(PreprocessResult {
        expanded_source,
        source_map,
    })
}
