use std::ops::Range;

/// Opaque handle into the file registry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub(crate) u32);

impl FileId {
    /// Get the numeric index for this file ID (for use as a map key)
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Entry for a loaded file
#[derive(Debug, Clone)]
struct FileEntry {
    path: String,
    content: String,
}

/// Registry of all source files encountered during preprocessing
#[derive(Debug, Clone, Default)]
pub struct FileRegistry {
    files: Vec<FileEntry>,
}

impl FileRegistry {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    /// Register a file and return its ID
    pub fn add(&mut self, path: &str, content: String) -> FileId {
        let id = FileId(self.files.len() as u32);
        self.files.push(FileEntry {
            path: path.to_string(),
            content,
        });
        id
    }

    /// Get the display path for a file
    pub fn path(&self, id: FileId) -> &str {
        &self.files[id.0 as usize].path
    }

    /// Get the original content of a file
    pub fn content(&self, id: FileId) -> &str {
        &self.files[id.0 as usize].content
    }

    /// Get all registered file IDs
    pub fn file_ids(&self) -> impl Iterator<Item = FileId> {
        (0..self.files.len()).map(|i| FileId(i as u32))
    }

    /// Compute the byte offset of the start of a 1-based line number in a file's content.
    /// Returns the byte offset, or 0 if the line is out of range.
    pub fn line_byte_offset(&self, id: FileId, line: u32) -> usize {
        let content = self.content(id);
        if line <= 1 {
            return 0;
        }
        let target = (line - 1) as usize;
        let mut current_line = 0;
        for (i, ch) in content.char_indices() {
            if ch == '\n' {
                current_line += 1;
                if current_line == target {
                    return i + 1;
                }
            }
        }
        // Line beyond end of file -- clamp to end
        content.len()
    }

    /// Get the length of a 1-based line in a file (excluding newline).
    pub fn line_length(&self, id: FileId, line: u32) -> usize {
        let content = self.content(id);
        let start = self.line_byte_offset(id, line);
        let rest = &content[start..];
        rest.find('\n').unwrap_or(rest.len())
    }
}

/// Information about a macro expansion in the call stack
#[derive(Debug, Clone)]
pub struct MacroExpansionInfo {
    pub macro_name: String,
    pub invocation_origin: SourceOrigin,
    pub depth: u32,
}

/// Where a line of expanded output originally came from
#[derive(Debug, Clone)]
pub struct SourceOrigin {
    pub file_id: FileId,
    /// 1-based line number in the original file
    pub line: u32,
    /// If this line was produced by a macro expansion, the chain of expansions
    pub macro_expansion: Option<Box<MacroExpansionInfo>>,
}

impl SourceOrigin {
    pub fn new(file_id: FileId, line: u32) -> Self {
        Self {
            file_id,
            line,
            macro_expansion: None,
        }
    }

    pub fn with_macro_expansion(
        file_id: FileId,
        line: u32,
        macro_name: String,
        invocation_origin: SourceOrigin,
        depth: u32,
    ) -> Self {
        Self {
            file_id,
            line,
            macro_expansion: Some(Box::new(MacroExpansionInfo {
                macro_name,
                invocation_origin,
                depth,
            })),
        }
    }
}

/// Maps byte offsets in expanded text back to original source locations.
///
/// The source map records one `SourceOrigin` per line of expanded output.
/// To resolve a byte offset from pest, we convert it to a line number
/// (by counting newlines), then look up the origin for that line.
#[derive(Debug, Clone)]
pub struct SourceMap {
    pub file_registry: FileRegistry,
    line_origins: Vec<SourceOrigin>,
}

impl SourceMap {
    pub fn new(file_registry: FileRegistry, line_origins: Vec<SourceOrigin>) -> Self {
        Self {
            file_registry,
            line_origins,
        }
    }

    /// Resolve a byte offset in the expanded source to its original location
    pub fn resolve(&self, byte_offset: usize, expanded_source: &str) -> &SourceOrigin {
        let line_index = byte_offset_to_line(byte_offset, expanded_source);
        let clamped = line_index.min(self.line_origins.len().saturating_sub(1));
        &self.line_origins[clamped]
    }

    /// Remap a span (byte range) from expanded source to an original origin.
    /// Uses the start of the span for resolution.
    pub fn resolve_span(&self, span: &Range<usize>, expanded_source: &str) -> &SourceOrigin {
        self.resolve(span.start, expanded_source)
    }

    /// Get the number of tracked output lines
    pub fn len(&self) -> usize {
        self.line_origins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.line_origins.is_empty()
    }

    /// Format a human-readable location string for diagnostics
    pub fn format_location(&self, origin: &SourceOrigin) -> String {
        let file_path = self.file_registry.path(origin.file_id);
        let mut location = format!("{}:{}", file_path, origin.line);

        if let Some(ref expansion) = origin.macro_expansion {
            location.push_str(&format!(
                " (in expansion of macro '{}'",
                expansion.macro_name
            ));
            let mut current = &expansion.invocation_origin;
            loop {
                let inv_path = self.file_registry.path(current.file_id);
                location.push_str(&format!(", invoked at {}:{}", inv_path, current.line));
                if let Some(ref inner) = current.macro_expansion {
                    location.push_str(&format!(", in expansion of macro '{}'", inner.macro_name));
                    current = &inner.invocation_origin;
                } else {
                    break;
                }
            }
            location.push(')');
        }

        location
    }
}

/// Convert a byte offset to a 0-based line index by counting newlines
fn byte_offset_to_line(byte_offset: usize, source: &str) -> usize {
    let clamped = byte_offset.min(source.len());
    source[..clamped].matches('\n').count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_registry() {
        let mut reg = FileRegistry::new();
        let id1 = reg.add("main.s", "line1\nline2".to_string());
        let id2 = reg.add("utils.s", "helper".to_string());

        assert_eq!(reg.path(id1), "main.s");
        assert_eq!(reg.path(id2), "utils.s");
        assert_eq!(reg.content(id1), "line1\nline2");
    }

    #[test]
    fn test_byte_offset_to_line() {
        let source = "line0\nline1\nline2\n";
        assert_eq!(byte_offset_to_line(0, source), 0);
        assert_eq!(byte_offset_to_line(3, source), 0);
        assert_eq!(byte_offset_to_line(6, source), 1);
        assert_eq!(byte_offset_to_line(12, source), 2);
        // Past end clamps
        assert_eq!(byte_offset_to_line(999, source), 3);
    }

    #[test]
    fn test_source_map_resolve() {
        let mut reg = FileRegistry::new();
        let file_id = reg.add("test.s", "original content".to_string());

        let origins = vec![
            SourceOrigin::new(file_id, 1),
            SourceOrigin::new(file_id, 5),
            SourceOrigin::new(file_id, 10),
        ];

        let expanded = "first line\nsecond line\nthird line\n";
        let map = SourceMap::new(reg, origins);

        // Offset in first line -> origin line 1
        let origin = map.resolve(3, expanded);
        assert_eq!(origin.line, 1);

        // Offset in second line -> origin line 5
        let origin = map.resolve(11, expanded);
        assert_eq!(origin.line, 5);

        // Offset in third line -> origin line 10
        let origin = map.resolve(23, expanded);
        assert_eq!(origin.line, 10);
    }

    #[test]
    fn test_format_location_simple() {
        let mut reg = FileRegistry::new();
        let file_id = reg.add("main.s", String::new());
        let origin = SourceOrigin::new(file_id, 42);
        let map = SourceMap::new(reg, vec![]);

        assert_eq!(map.format_location(&origin), "main.s:42");
    }

    #[test]
    fn test_format_location_with_macro() {
        let mut reg = FileRegistry::new();
        let file_id = reg.add("macros.s", String::new());
        let main_id = reg.add("main.s", String::new());

        let origin = SourceOrigin::with_macro_expansion(
            file_id,
            3,
            "MY_MACRO".to_string(),
            SourceOrigin::new(main_id, 15),
            1,
        );
        let map = SourceMap::new(reg, vec![]);

        assert_eq!(
            map.format_location(&origin),
            "macros.s:3 (in expansion of macro 'MY_MACRO', invoked at main.s:15)"
        );
    }
}
