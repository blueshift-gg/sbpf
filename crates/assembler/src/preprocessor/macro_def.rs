use {
    crate::errors::CompileError,
    super::SourceLine,
    super::source_map::SourceOrigin,
};

/// A macro parameter
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub default: Option<String>,
    pub is_vararg: bool,
}

/// A parsed macro definition
#[derive(Debug, Clone)]
pub struct MacroDef {
    pub name: String,
    pub params: Vec<Param>,
    pub body_lines: Vec<String>,
    pub defined_at: SourceOrigin,
}

/// Result of scanning lines for macro definitions
pub(crate) struct MacroScanResult {
    /// All macro definitions found, keyed by name
    pub macros: std::collections::HashMap<String, MacroDef>,
    /// Lines that are not part of macro definitions (pass-through)
    pub remaining_lines: Vec<SourceLine>,
    /// Any errors encountered during scanning, paired with their source origin
    pub errors: Vec<(CompileError, SourceOrigin)>,
}

/// Scan lines for `.macro` / `.endm` blocks, extracting definitions.
///
/// Lines between `.macro` and `.endm` become the macro body.
/// All other lines are returned in `remaining_lines` for further processing.
pub(crate) fn scan_macro_definitions(lines: Vec<SourceLine>) -> MacroScanResult {
    let mut macros = std::collections::HashMap::new();
    let mut remaining = Vec::new();
    let mut errors = Vec::new();

    let mut current_macro: Option<(String, Vec<Param>, SourceOrigin, Vec<String>)> = None;

    // Track the origin of the .macro directive line for error reporting
    let mut macro_start_origin: Option<SourceOrigin> = None;

    for line in lines {
        let trimmed = line.text.trim();

        if let Some(ref mut macro_state) = current_macro {
            if trimmed == ".endm" {
                // Close the macro definition
                let (name, params, origin, body) = current_macro.take().unwrap();
                macro_start_origin = None;

                if macros.contains_key(&name) {
                    errors.push((
                        CompileError::DuplicateMacroDef {
                            name: name.clone(),
                            span: 0..0,
                            custom_label: None,
                        },
                        origin,
                    ));
                } else {
                    macros.insert(
                        name.clone(),
                        MacroDef {
                            name,
                            params,
                            body_lines: body,
                            defined_at: origin,
                        },
                    );
                }
            } else {
                // Accumulate body lines
                macro_state.3.push(line.text.clone());
            }
        } else if let Some((name, params)) = parse_macro_directive(trimmed) {
            // Validate parameters
            let mut has_errors = false;
            let mut seen_vararg = false;
            for (i, param) in params.iter().enumerate() {
                if param.is_vararg {
                    if seen_vararg {
                        errors.push((
                            CompileError::MultipleVararg {
                                name: name.clone(),
                                span: 0..0,
                                custom_label: None,
                            },
                            line.origin.clone(),
                        ));
                        has_errors = true;
                        break;
                    }
                    if i != params.len() - 1 {
                        errors.push((
                            CompileError::VarargNotLast {
                                name: name.clone(),
                                span: 0..0,
                                custom_label: None,
                            },
                            line.origin.clone(),
                        ));
                        has_errors = true;
                        break;
                    }
                    seen_vararg = true;
                }
            }
            if !has_errors {
                macro_start_origin = Some(line.origin.clone());
                current_macro = Some((name, params, line.origin.clone(), Vec::new()));
            }
        } else {
            remaining.push(line);
        }
    }

    // Check for unclosed macro at end of input
    if let Some((name, _, _, _)) = current_macro {
        let origin = macro_start_origin.unwrap_or(SourceOrigin::new(
            super::source_map::FileId(0),
            0,
        ));
        errors.push((
            CompileError::UnclosedMacro {
                name,
                span: 0..0,
                custom_label: None,
            },
            origin,
        ));
    }

    MacroScanResult {
        macros,
        remaining_lines: remaining,
        errors,
    }
}

/// Parse a `.macro name param1, param2=default, param3:vararg` directive.
/// Returns the macro name and parsed parameters.
fn parse_macro_directive(line: &str) -> Option<(String, Vec<Param>)> {
    let rest = line.strip_prefix(".macro")?;

    // Must have whitespace after .macro
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }

    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }

    // First token is the macro name
    let (name, params_str) = match rest.find(char::is_whitespace) {
        Some(pos) => (&rest[..pos], rest[pos..].trim()),
        None => (rest, ""),
    };

    let params = if params_str.is_empty() {
        Vec::new()
    } else {
        parse_params(params_str)?
    };

    Some((name.to_string(), params))
}

/// Parse a comma-separated parameter list.
/// Supports: `name`, `name=default`, `name:vararg`
fn parse_params(params_str: &str) -> Option<Vec<Param>> {
    let mut params = Vec::new();

    for part in params_str.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some((name, _)) = part.split_once(":vararg") {
            let name = name.trim();
            params.push(Param {
                name: name.to_string(),
                default: None,
                is_vararg: true,
            });
        } else if let Some((name, default)) = part.split_once('=') {
            let name = name.trim();
            let default = default.trim();
            params.push(Param {
                name: name.to_string(),
                default: Some(default.to_string()),
                is_vararg: false,
            });
        } else {
            params.push(Param {
                name: part.to_string(),
                default: None,
                is_vararg: false,
            });
        }
    }

    Some(params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preprocessor::source_map::{FileId, SourceOrigin};

    fn make_source_line(text: &str, line: u32) -> SourceLine {
        SourceLine {
            text: text.to_string(),
            origin: SourceOrigin::new(FileId(0), line),
        }
    }

    #[test]
    fn test_parse_macro_directive_simple() {
        let (name, params) = parse_macro_directive(".macro MY_MACRO").unwrap();
        assert_eq!(name, "MY_MACRO");
        assert!(params.is_empty());
    }

    #[test]
    fn test_parse_macro_directive_with_params() {
        let (name, params) = parse_macro_directive(".macro PUSH reg").unwrap();
        assert_eq!(name, "PUSH");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "reg");
        assert!(!params[0].is_vararg);
        assert!(params[0].default.is_none());
    }

    #[test]
    fn test_parse_macro_directive_with_defaults() {
        let (name, params) = parse_macro_directive(".macro FOO a, b=42").unwrap();
        assert_eq!(name, "FOO");
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, "a");
        assert!(params[0].default.is_none());
        assert_eq!(params[1].name, "b");
        assert_eq!(params[1].default.as_deref(), Some("42"));
    }

    #[test]
    fn test_parse_macro_directive_with_vararg() {
        let (name, params) = parse_macro_directive(".macro LOG fmt, args:vararg").unwrap();
        assert_eq!(name, "LOG");
        assert_eq!(params.len(), 2);
        assert!(!params[0].is_vararg);
        assert!(params[1].is_vararg);
        assert_eq!(params[1].name, "args");
    }

    #[test]
    fn test_parse_macro_directive_not_macro() {
        assert!(parse_macro_directive("mov64 r1, 1").is_none());
        assert!(parse_macro_directive(".macros FOO").is_none());
        assert!(parse_macro_directive(".macro").is_none());
    }

    #[test]
    fn test_scan_simple_macro() {
        let lines = vec![
            make_source_line("before", 1),
            make_source_line(".macro PUSH reg", 2),
            make_source_line("    stxdw [r10-8], \\reg", 3),
            make_source_line("    add64 r10, -8", 4),
            make_source_line(".endm", 5),
            make_source_line("after", 6),
        ];

        let result = scan_macro_definitions(lines);
        assert!(result.errors.is_empty());
        assert_eq!(result.macros.len(), 1);
        assert!(result.macros.contains_key("PUSH"));

        let macro_def = &result.macros["PUSH"];
        assert_eq!(macro_def.params.len(), 1);
        assert_eq!(macro_def.params[0].name, "reg");
        assert_eq!(macro_def.body_lines.len(), 2);

        assert_eq!(result.remaining_lines.len(), 2);
        assert_eq!(result.remaining_lines[0].text, "before");
        assert_eq!(result.remaining_lines[1].text, "after");
    }

    #[test]
    fn test_scan_unclosed_macro() {
        let lines = vec![
            make_source_line(".macro BAD", 1),
            make_source_line("    body", 2),
        ];

        let result = scan_macro_definitions(lines);
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            &result.errors[0].0,
            CompileError::UnclosedMacro { name, .. } if name == "BAD"
        ));
        // Origin should point to line 1 where .macro was declared
        assert_eq!(result.errors[0].1.line, 1);
    }

    #[test]
    fn test_scan_duplicate_macro() {
        let lines = vec![
            make_source_line(".macro FOO", 1),
            make_source_line(".endm", 2),
            make_source_line(".macro FOO", 3),
            make_source_line(".endm", 4),
        ];

        let result = scan_macro_definitions(lines);
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            &result.errors[0].0,
            CompileError::DuplicateMacroDef { name, .. } if name == "FOO"
        ));
    }

    #[test]
    fn test_scan_vararg_not_last() {
        let lines = vec![
            make_source_line(".macro BAD args:vararg, extra", 1),
            make_source_line(".endm", 2),
        ];

        let result = scan_macro_definitions(lines);
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            &result.errors[0].0,
            CompileError::VarargNotLast { .. }
        ));
    }
}
