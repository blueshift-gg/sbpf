use {
    super::{
        FileResolver, SourceLine,
        source_map::{FileRegistry, SourceOrigin},
    },
    crate::errors::CompileError,
    std::collections::HashSet,
};

/// Parse a `.include "path"` directive from a line.
/// Returns Some(path) if the line is an include directive, None otherwise.
fn parse_include_directive(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix(".include")?;

    // Must have whitespace after .include
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }

    let rest = rest.trim();

    // Path must be quoted
    if rest.starts_with('"') && rest.ends_with('"') && rest.len() >= 2 {
        Some(&rest[1..rest.len() - 1])
    } else {
        None
    }
}

/// Resolve all `.include` directives recursively, producing a flat list of source lines.
///
/// Each line tracks its origin (file + line number) for diagnostics.
/// Cycle detection uses a stack-based approach: A including B including A is caught,
/// but A including B, then A including C (which also includes B) is allowed.
pub(crate) fn resolve_includes(
    source: &str,
    source_path: &str,
    resolver: Option<&dyn FileResolver>,
    registry: &mut FileRegistry,
) -> Result<Vec<SourceLine>, Vec<CompileError>> {
    let mut include_stack: HashSet<String> = HashSet::new();
    include_stack.insert(source_path.to_string());

    let file_id = registry.add(source_path, source.to_string());
    let mut errors = Vec::new();
    let mut output = Vec::new();

    resolve_recursive(
        source,
        file_id,
        source_path,
        resolver,
        registry,
        &mut include_stack,
        &mut output,
        &mut errors,
    );

    if errors.is_empty() {
        Ok(output)
    } else {
        Err(errors)
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_recursive(
    source: &str,
    file_id: super::source_map::FileId,
    file_path: &str,
    resolver: Option<&dyn FileResolver>,
    registry: &mut FileRegistry,
    include_stack: &mut HashSet<String>,
    output: &mut Vec<SourceLine>,
    errors: &mut Vec<CompileError>,
) {
    for (line_idx, line_text) in source.lines().enumerate() {
        let line_number = (line_idx + 1) as u32;

        if let Some(include_path) = parse_include_directive(line_text) {
            // Calculate a span for error reporting.
            // We use the byte offset of this line in the source.
            let line_start: usize = source
                .lines()
                .take(line_idx)
                .map(|l| l.len() + 1) // +1 for newline
                .sum();
            let span = line_start..line_start + line_text.len();

            let resolver = match resolver {
                Some(r) => r,
                None => {
                    errors.push(CompileError::IncludeNotFound {
                        path: include_path.to_string(),
                        span,
                        custom_label: Some("No file resolver configured".to_string()),
                    });
                    continue;
                }
            };

            // Check for include cycle
            if include_stack.contains(include_path) {
                errors.push(CompileError::IncludeCycle {
                    path: include_path.to_string(),
                    span,
                    custom_label: None,
                });
                continue;
            }

            // Resolve and read the file
            match resolver.resolve(include_path, file_path) {
                Ok(content) => {
                    let included_file_id = registry.add(include_path, content.clone());
                    include_stack.insert(include_path.to_string());

                    resolve_recursive(
                        &content,
                        included_file_id,
                        include_path,
                        Some(resolver),
                        registry,
                        include_stack,
                        output,
                        errors,
                    );

                    include_stack.remove(include_path);
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        errors.push(CompileError::IncludeNotFound {
                            path: include_path.to_string(),
                            span,
                            custom_label: None,
                        });
                    } else {
                        errors.push(CompileError::IncludeReadError {
                            path: include_path.to_string(),
                            reason: e.to_string(),
                            span,
                            custom_label: None,
                        });
                    }
                }
            }
        } else {
            // Regular line -- pass through with origin tracking
            output.push(SourceLine {
                text: line_text.to_string(),
                origin: SourceOrigin::new(file_id, line_number),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::preprocessor::MockFileResolver};

    #[test]
    fn test_parse_include_directive() {
        assert_eq!(parse_include_directive(".include \"foo.s\""), Some("foo.s"));
        assert_eq!(
            parse_include_directive("  .include \"path/to/file.s\"  "),
            Some("path/to/file.s")
        );
        assert_eq!(parse_include_directive(".include foo.s"), None);
        assert_eq!(parse_include_directive("mov64 r1, 1"), None);
        assert_eq!(parse_include_directive(".includes \"foo.s\""), None);
        assert_eq!(parse_include_directive(".include"), None);
    }

    #[test]
    fn test_no_includes() {
        let source = "mov64 r1, 1\nexit\n";
        let mut registry = FileRegistry::new();
        let result = resolve_includes(source, "<input>", None, &mut registry).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].text, "mov64 r1, 1");
        assert_eq!(result[0].origin.line, 1);
        assert_eq!(result[1].text, "exit");
        assert_eq!(result[1].origin.line, 2);
    }

    #[test]
    fn test_single_include() {
        let mut resolver = MockFileResolver::new();
        resolver.add_file("macros.s", "macro_line1\nmacro_line2");

        let source = "before\n.include \"macros.s\"\nafter";
        let mut registry = FileRegistry::new();
        let result = resolve_includes(source, "<input>", Some(&resolver), &mut registry).unwrap();

        assert_eq!(result.len(), 4);
        assert_eq!(result[0].text, "before");
        assert_eq!(result[1].text, "macro_line1");
        assert_eq!(result[2].text, "macro_line2");
        assert_eq!(result[3].text, "after");

        // Check origin tracking
        assert_eq!(registry.path(result[0].origin.file_id), "<input>");
        assert_eq!(registry.path(result[1].origin.file_id), "macros.s");
        assert_eq!(result[1].origin.line, 1);
        assert_eq!(result[2].origin.line, 2);
    }

    #[test]
    fn test_nested_includes() {
        let mut resolver = MockFileResolver::new();
        resolver.add_file("a.s", "a_line\n.include \"b.s\"\na_end");
        resolver.add_file("b.s", "b_line");

        let source = ".include \"a.s\"";
        let mut registry = FileRegistry::new();
        let result = resolve_includes(source, "<input>", Some(&resolver), &mut registry).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].text, "a_line");
        assert_eq!(result[1].text, "b_line");
        assert_eq!(result[2].text, "a_end");
    }

    #[test]
    fn test_include_cycle_detection() {
        let mut resolver = MockFileResolver::new();
        resolver.add_file("a.s", ".include \"<input>\"");

        let source = ".include \"a.s\"";
        let mut registry = FileRegistry::new();
        let result = resolve_includes(source, "<input>", Some(&resolver), &mut registry);

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], CompileError::IncludeCycle { path, .. } if path == "<input>"));
    }

    #[test]
    fn test_include_not_found() {
        let resolver = MockFileResolver::new();

        let source = ".include \"missing.s\"";
        let mut registry = FileRegistry::new();
        let result = resolve_includes(source, "<input>", Some(&resolver), &mut registry);

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            matches!(&errors[0], CompileError::IncludeNotFound { path, .. } if path == "missing.s")
        );
    }

    #[test]
    fn test_diamond_include_allowed() {
        // A includes B and C, both B and C include D -- this is allowed (not a cycle)
        let mut resolver = MockFileResolver::new();
        resolver.add_file("b.s", ".include \"d.s\"\nb_line");
        resolver.add_file("c.s", ".include \"d.s\"\nc_line");
        resolver.add_file("d.s", "d_line");

        let source = ".include \"b.s\"\n.include \"c.s\"";
        let mut registry = FileRegistry::new();
        let result = resolve_includes(source, "<input>", Some(&resolver), &mut registry).unwrap();

        assert_eq!(result.len(), 4);
        assert_eq!(result[0].text, "d_line");
        assert_eq!(result[1].text, "b_line");
        assert_eq!(result[2].text, "d_line");
        assert_eq!(result[3].text, "c_line");
    }
}
