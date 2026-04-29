use {
    super::{
        SourceLine,
        macro_def::{MacroDef, scan_macro_definitions},
        source_map::SourceOrigin,
    },
    crate::errors::CompileError,
    std::{
        collections::HashMap,
        sync::atomic::{AtomicU64, Ordering},
    },
};

const MAX_EXPANSION_DEPTH: u32 = 100;

/// Global expansion counter for `\@` unique IDs
static EXPANSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Reset the expansion counter (for testing)
#[cfg(test)]
pub(crate) fn reset_expansion_counter() {
    EXPANSION_COUNTER.store(0, Ordering::Relaxed);
}

/// An expansion error paired with its source origin
#[derive(Debug)]
pub(crate) struct ExpandError {
    pub error: CompileError,
    pub origin: Option<SourceOrigin>,
}

/// Expand all macros, `.rept`, and `.irp` directives in the given lines.
///
/// Returns the expanded lines and any errors encountered.
pub(crate) fn expand_macros(
    lines: Vec<SourceLine>,
) -> Result<(Vec<SourceLine>, Vec<ExpandError>), Vec<ExpandError>> {
    // Scan for macro definitions and separate them from regular lines
    let scan_result = scan_macro_definitions(lines);

    if !scan_result.errors.is_empty() {
        return Err(scan_result
            .errors
            .into_iter()
            .map(|(error, origin)| ExpandError {
                error,
                origin: Some(origin),
            })
            .collect());
    }

    let macros = scan_result.macros;
    let mut errors = Vec::new();
    let mut output = Vec::new();

    // Expand macro invocations in remaining lines
    for line in scan_result.remaining_lines {
        expand_line(&line, &macros, &mut output, &mut errors, 0);
    }

    // Now, handle .rept and .irp on the post-macro output
    let output = expand_repetitions(output)?;

    Ok((output, errors))
}

/// Expand a single line, checking if it's a macro invocation.
fn expand_line(
    line: &SourceLine,
    macros: &HashMap<String, MacroDef>,
    output: &mut Vec<SourceLine>,
    errors: &mut Vec<ExpandError>,
    depth: u32,
) {
    if depth > MAX_EXPANSION_DEPTH {
        errors.push(ExpandError {
            error: CompileError::MacroRecursionLimit {
                limit: MAX_EXPANSION_DEPTH,
                span: 0..0,
                custom_label: None,
            },
            origin: Some(line.origin.clone()),
        });
        return;
    }

    let trimmed = line.text.trim();

    // Check if the first token matches a known macro name
    let first_token = first_token(trimmed);
    if let Some(macro_def) = first_token.and_then(|name| macros.get(name)) {
        let args_str = trimmed[macro_def.name.len()..].trim();
        let args = split_args(args_str);

        match bind_args(macro_def, &args) {
            Ok(bindings) => {
                let expansion_id = EXPANSION_COUNTER.fetch_add(1, Ordering::Relaxed);

                // Expand each body line with parameter substitution
                for body_line in &macro_def.body_lines {
                    let expanded_text = substitute(body_line, &bindings, expansion_id);
                    let expanded_line = SourceLine {
                        text: expanded_text,
                        origin: SourceOrigin::with_macro_expansion(
                            macro_def.defined_at.file_id,
                            macro_def.defined_at.line,
                            macro_def.name.clone(),
                            line.origin.clone(),
                            depth + 1,
                        ),
                    };

                    // Rescan for further macro invocations
                    expand_line(&expanded_line, macros, output, errors, depth + 1);
                }
            }
            Err(e) => errors.push(ExpandError {
                error: e,
                origin: Some(line.origin.clone()),
            }),
        }
    } else {
        // Not a macro invocation, pass through
        output.push(line.clone());
    }
}

/// Extract the first whitespace-delimited token from a line.
/// Skips labels (tokens ending with ':').
fn first_token(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
        return None;
    }

    // If line starts with '.', it's a directive, not a macro
    if trimmed.starts_with('.') {
        return None;
    }

    let end = trimmed
        .find(|c: char| c.is_whitespace() || c == ',')
        .unwrap_or(trimmed.len());
    let token = &trimmed[..end];

    // Skip labels (they end with ':')
    if token.ends_with(':') {
        // Check if there's something after the label on the same line
        let rest = trimmed[end..].trim();
        if rest.is_empty() {
            return None;
        }
        return first_token(rest);
    }

    Some(token)
}

/// Split arguments by top-level commas, respecting quotes and brackets.
fn split_args(args_str: &str) -> Vec<String> {
    if args_str.is_empty() {
        return Vec::new();
    }

    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32; // bracket depth
    let mut in_string = false;
    let mut escape_next = false;

    for ch in args_str.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }

        match ch {
            '\\' if in_string => {
                current.push(ch);
                escape_next = true;
            }
            '"' => {
                in_string = !in_string;
                current.push(ch);
            }
            '(' | '[' if !in_string => {
                depth += 1;
                current.push(ch);
            }
            ')' | ']' if !in_string => {
                depth -= 1;
                current.push(ch);
            }
            ',' if !in_string && depth == 0 => {
                args.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    let last = current.trim().to_string();
    if !last.is_empty() {
        args.push(last);
    }

    args
}

/// Bind arguments to macro parameters, producing a name->value map.
fn bind_args(
    macro_def: &MacroDef,
    args: &[String],
) -> Result<HashMap<String, String>, CompileError> {
    let mut bindings = HashMap::new();

    let required_count = macro_def
        .params
        .iter()
        .filter(|p| p.default.is_none() && !p.is_vararg)
        .count();

    let has_vararg = macro_def.params.iter().any(|p| p.is_vararg);

    // Check argument count
    if !has_vararg && args.len() > macro_def.params.len() {
        return Err(CompileError::MacroArgCount {
            name: macro_def.name.clone(),
            expected: macro_def.params.len(),
            got: args.len(),
            span: 0..0,
            custom_label: None,
        });
    }
    if args.len() < required_count {
        return Err(CompileError::MacroArgCount {
            name: macro_def.name.clone(),
            expected: required_count,
            got: args.len(),
            span: 0..0,
            custom_label: None,
        });
    }

    for (i, param) in macro_def.params.iter().enumerate() {
        if param.is_vararg {
            // Collect all remaining arguments
            let vararg_values: Vec<&str> = args[i..].iter().map(|s| s.as_str()).collect();
            bindings.insert(param.name.clone(), vararg_values.join(", "));
        } else if i < args.len() {
            bindings.insert(param.name.clone(), args[i].clone());
        } else if let Some(ref default) = param.default {
            bindings.insert(param.name.clone(), default.clone());
        }
        // required params without args already caught above
    }

    // Also add positional references (\1, \2, etc.)
    for (i, arg) in args.iter().enumerate() {
        bindings.insert((i + 1).to_string(), arg.clone());
    }

    Ok(bindings)
}

/// Perform parameter substitution on a single body line.
///
/// Handles:
/// - `\name` -> argument value
/// - `\@` -> unique expansion ID
/// - `\()` -> zero-width concatenation (disappears after adjacent substitution)
/// - `\\` -> literal `\`
fn substitute(line: &str, bindings: &HashMap<String, String>, expansion_id: u64) -> String {
    let mut result = String::with_capacity(line.len());
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '\\' && i + 1 < len {
            let next = chars[i + 1];

            if next == '\\' {
                // Escaped backslash
                result.push('\\');
                i += 2;
            } else if next == '@' {
                // Unique expansion ID
                result.push_str(&expansion_id.to_string());
                i += 2;
            } else if next == '(' && i + 2 < len && chars[i + 2] == ')' {
                // Token concatenation -- just skip it (zero-width separator)
                i += 3;
            } else if next.is_alphanumeric() || next == '_' {
                // Parameter reference: collect the name
                let start = i + 1;
                let mut end = start;
                while end < len && (chars[end].is_alphanumeric() || chars[end] == '_') {
                    end += 1;
                }
                let param_name: String = chars[start..end].iter().collect();

                if let Some(value) = bindings.get(&param_name) {
                    result.push_str(value);
                    // Check if followed by \() for concatenation
                    if end + 2 < len
                        && chars[end] == '\\'
                        && chars[end + 1] == '('
                        && chars[end + 2] == ')'
                    {
                        // Skip the \() separator
                        end += 3;
                    }
                } else {
                    // Unknown parameter -- keep as-is
                    result.push('\\');
                    result.push_str(&param_name);
                }
                i = end;
            } else {
                // Unknown escape, keep as-is
                result.push('\\');
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

/// Expand `.rept` and `.irp` blocks (these are processed before macro expansion).
fn expand_repetitions(lines: Vec<SourceLine>) -> Result<Vec<SourceLine>, Vec<ExpandError>> {
    let mut output = Vec::new();
    let mut errors = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].text.trim();

        if let Some(count_str) = trimmed.strip_prefix(".rept").and_then(|r| {
            if r.starts_with(char::is_whitespace) {
                Some(r.trim())
            } else {
                None
            }
        }) {
            // Find matching .endr
            let (body, end_idx) = find_endr_block(&lines, i + 1, &lines[i].origin)?;

            match count_str.parse::<usize>() {
                Ok(count) => {
                    // Recursively expand nested .rept/.irp inside the body
                    let expanded_body = expand_repetitions(body)?;
                    for _ in 0..count {
                        output.extend(expanded_body.iter().cloned());
                    }
                }
                Err(_) => {
                    errors.push(ExpandError {
                        error: CompileError::InvalidReptCount {
                            value: count_str.to_string(),
                            span: 0..0,
                            custom_label: None,
                        },
                        origin: Some(lines[i].origin.clone()),
                    });
                }
            }

            i = end_idx + 1;
        } else if let Some(rest) = trimmed.strip_prefix(".irp").and_then(|r| {
            if r.starts_with(char::is_whitespace) {
                Some(r.trim())
            } else {
                None
            }
        }) {
            // Parse: .irp var, val1, val2, val3
            let (body, end_idx) = find_endr_block(&lines, i + 1, &lines[i].origin)?;

            if let Some((var_name, values_str)) = rest.split_once(',') {
                let var_name = var_name.trim();
                let values = split_args(values_str.trim());

                for value in &values {
                    let mut bindings = HashMap::new();
                    bindings.insert(var_name.to_string(), value.clone());

                    // Substitute for this iteration first, then recursively
                    // expand any nested .rept/.irp in the result.
                    let mut iter_lines = Vec::with_capacity(body.len());
                    for body_line in &body {
                        let expansion_id = EXPANSION_COUNTER.fetch_add(1, Ordering::Relaxed);
                        let expanded_text = substitute(&body_line.text, &bindings, expansion_id);
                        iter_lines.push(SourceLine {
                            text: expanded_text,
                            origin: body_line.origin.clone(),
                        });
                    }
                    output.extend(expand_repetitions(iter_lines)?);
                }
            }

            i = end_idx + 1;
        } else {
            output.push(lines[i].clone());
            i += 1;
        }
    }

    if errors.is_empty() {
        Ok(output)
    } else {
        Err(errors)
    }
}

/// Find the matching `.endr` for a `.rept` or `.irp` block, handling nesting.
fn find_endr_block(
    lines: &[SourceLine],
    start: usize,
    directive_origin: &SourceOrigin,
) -> Result<(Vec<SourceLine>, usize), Vec<ExpandError>> {
    let mut depth = 1u32;
    let mut i = start;

    while i < lines.len() {
        let trimmed = lines[i].text.trim();
        if trimmed == ".endr" {
            depth -= 1;
            if depth == 0 {
                let body = lines[start..i].to_vec();
                return Ok((body, i));
            }
        } else if trimmed.starts_with(".rept") || trimmed.starts_with(".irp") {
            depth += 1;
        }
        i += 1;
    }

    Err(vec![ExpandError {
        error: CompileError::UnclosedRept {
            span: 0..0,
            custom_label: None,
        },
        origin: Some(directive_origin.clone()),
    }])
}

#[cfg(test)]
mod tests {
    use {super::*, crate::preprocessor::source_map::FileId};

    fn make_line(text: &str, line: u32) -> SourceLine {
        SourceLine {
            text: text.to_string(),
            origin: SourceOrigin::new(FileId(0), line),
        }
    }

    fn expand_and_collect(lines: Vec<SourceLine>) -> Vec<String> {
        let (result, errors) = expand_macros(lines).unwrap();
        assert!(errors.is_empty(), "Unexpected errors: {:?}", errors);
        result.iter().map(|l| l.text.clone()).collect()
    }

    #[test]
    fn test_first_token() {
        assert_eq!(first_token("MY_MACRO arg1, arg2"), Some("MY_MACRO"));
        assert_eq!(first_token("  MY_MACRO  "), Some("MY_MACRO"));
        assert_eq!(first_token(".globl entry"), None); // directive
        assert_eq!(first_token("# comment"), None);
        assert_eq!(first_token(""), None);
        assert_eq!(first_token("label:"), None);
        assert_eq!(first_token("label: MY_MACRO"), Some("MY_MACRO"));
    }

    #[test]
    fn test_split_args() {
        assert_eq!(split_args("a, b, c"), vec!["a", "b", "c"]);
        assert_eq!(split_args("r1, 42"), vec!["r1", "42"]);
        assert_eq!(
            split_args("\"hello, world\", 1"),
            vec!["\"hello, world\"", "1"]
        );
        assert_eq!(split_args(""), Vec::<String>::new());
        assert_eq!(split_args("single"), vec!["single"]);
    }

    #[test]
    fn test_substitute_simple() {
        let mut bindings = HashMap::new();
        bindings.insert("reg".to_string(), "r1".to_string());
        bindings.insert("val".to_string(), "42".to_string());

        assert_eq!(
            substitute("    mov64 \\reg, \\val", &bindings, 0),
            "    mov64 r1, 42"
        );
    }

    #[test]
    fn test_substitute_unique_id() {
        let bindings = HashMap::new();
        assert_eq!(substitute("label_\\@:", &bindings, 5), "label_5:");
    }

    #[test]
    fn test_substitute_concatenation() {
        let mut bindings = HashMap::new();
        bindings.insert("msg".to_string(), "e1".to_string());

        assert_eq!(substitute("\\msg\\()_end", &bindings, 0), "e1_end");
    }

    #[test]
    fn test_substitute_escaped_backslash() {
        let bindings = HashMap::new();
        assert_eq!(substitute("\\\\n", &bindings, 0), "\\n");
    }

    #[test]
    fn test_simple_macro_expansion() {
        reset_expansion_counter();
        let lines = vec![
            make_line(".macro NOP", 1),
            make_line("    mov64 r0, 0", 2),
            make_line(".endm", 3),
            make_line("NOP", 4),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(result, vec!["    mov64 r0, 0"]);
    }

    #[test]
    fn test_macro_with_args() {
        reset_expansion_counter();
        let lines = vec![
            make_line(".macro MOV dst, src", 1),
            make_line("    mov64 \\dst, \\src", 2),
            make_line(".endm", 3),
            make_line("MOV r1, r2", 4),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(result, vec!["    mov64 r1, r2"]);
    }

    #[test]
    fn test_macro_with_default() {
        reset_expansion_counter();
        let lines = vec![
            make_line(".macro SET reg, val=0", 1),
            make_line("    mov64 \\reg, \\val", 2),
            make_line(".endm", 3),
            make_line("SET r1", 4),
            make_line("SET r2, 42", 5),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(result, vec!["    mov64 r1, 0", "    mov64 r2, 42"]);
    }

    #[test]
    fn test_macro_unique_id() {
        reset_expansion_counter();
        let lines = vec![
            make_line(".macro LOOP", 1),
            make_line("loop_\\@:", 2),
            make_line(".endm", 3),
            make_line("LOOP", 4),
            make_line("LOOP", 5),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|s| {
            s.starts_with("loop_")
                && s.ends_with(':')
                && s["loop_".len()..s.len() - 1].parse::<u64>().is_ok()
        }));
        assert_ne!(result[0], result[1]);
    }

    #[test]
    fn test_macro_concatenation() {
        reset_expansion_counter();
        let lines = vec![
            make_line(".macro DEF_STR name, text", 1),
            make_line("\\name:", 2),
            make_line("    .ascii \\text", 3),
            make_line("\\name\\()_end:", 4),
            make_line(".endm", 5),
            make_line("DEF_STR e1, \"error\"", 6),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(result, vec!["e1:", "    .ascii \"error\"", "e1_end:"]);
    }

    #[test]
    fn test_nested_macro_expansion() {
        reset_expansion_counter();
        let lines = vec![
            make_line(".macro INNER val", 1),
            make_line("    mov64 r0, \\val", 2),
            make_line(".endm", 3),
            make_line(".macro OUTER val", 4),
            make_line("INNER \\val", 5),
            make_line(".endm", 6),
            make_line("OUTER 42", 7),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(result, vec!["    mov64 r0, 42"]);
    }

    #[test]
    fn test_recursion_limit() {
        let lines = vec![
            make_line(".macro FOREVER", 1),
            make_line("FOREVER", 2),
            make_line(".endm", 3),
            make_line("FOREVER", 4),
        ];

        let (_, errors) = expand_macros(lines).unwrap();
        assert!(!errors.is_empty());
        assert!(matches!(
            &errors[0].error,
            CompileError::MacroRecursionLimit { .. }
        ));
        // Should have origin pointing to the invocation
        assert!(errors[0].origin.is_some());
    }

    #[test]
    fn test_wrong_arg_count() {
        let lines = vec![
            make_line(".macro NEED_TWO a, b", 1),
            make_line("    mov64 \\a, \\b", 2),
            make_line(".endm", 3),
            make_line("NEED_TWO r1", 4),
        ];

        let (_, errors) = expand_macros(lines).unwrap();
        assert!(!errors.is_empty());
        assert!(matches!(
            &errors[0].error,
            CompileError::MacroArgCount {
                expected: 2,
                got: 1,
                ..
            }
        ));
        assert!(errors[0].origin.is_some());
        assert_eq!(errors[0].origin.as_ref().unwrap().line, 4);
    }

    #[test]
    fn test_vararg() {
        reset_expansion_counter();
        let lines = vec![
            make_line(".macro LOG fmt, args:vararg", 1),
            make_line("    .ascii \\fmt", 2),
            make_line("    .ascii \\args", 3),
            make_line(".endm", 4),
            make_line("LOG \"hello\", a, b, c", 5),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(result, vec!["    .ascii \"hello\"", "    .ascii a, b, c"]);
    }

    #[test]
    fn test_rept() {
        let lines = vec![
            make_line(".rept 3", 1),
            make_line("    nop", 2),
            make_line(".endr", 3),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(result, vec!["    nop", "    nop", "    nop"]);
    }

    #[test]
    fn test_rept_zero() {
        let lines = vec![
            make_line(".rept 0", 1),
            make_line("    nop", 2),
            make_line(".endr", 3),
        ];

        let result = expand_and_collect(lines);
        assert!(result.is_empty());
    }

    #[test]
    fn test_irp() {
        reset_expansion_counter();
        let lines = vec![
            make_line(".irp reg, r1, r2, r3", 1),
            make_line("    mov64 \\reg, 0", 2),
            make_line(".endr", 3),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(
            result,
            vec!["    mov64 r1, 0", "    mov64 r2, 0", "    mov64 r3, 0"]
        );
    }

    #[test]
    fn test_nested_rept() {
        let lines = vec![
            make_line(".rept 2", 1),
            make_line("    .rept 3", 2),
            make_line("        mov64 r1, 0x1", 3),
            make_line("    .endr", 4),
            make_line(".endr", 5),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(result.len(), 6);
        assert!(result.iter().all(|s| s == "        mov64 r1, 0x1"));
    }

    #[test]
    fn test_nested_irp() {
        reset_expansion_counter();
        let lines = vec![
            make_line(".irp reg, r1, r2", 1),
            make_line("    .irp val, 0x1, 0x2", 2),
            make_line("        mov64 \\reg, \\val", 3),
            make_line("    .endr", 4),
            make_line(".endr", 5),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(
            result,
            vec![
                "        mov64 r1, 0x1",
                "        mov64 r1, 0x2",
                "        mov64 r2, 0x1",
                "        mov64 r2, 0x2",
            ]
        );
    }

    #[test]
    fn test_rept_inside_irp() {
        reset_expansion_counter();
        let lines = vec![
            make_line(".irp r, r1, r2", 1),
            make_line("    .rept 2", 2),
            make_line("        mov64 \\r, 0x1", 3),
            make_line("    .endr", 4),
            make_line(".endr", 5),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(
            result,
            vec![
                "        mov64 r1, 0x1",
                "        mov64 r1, 0x1",
                "        mov64 r2, 0x1",
                "        mov64 r2, 0x1",
            ]
        );
    }

    #[test]
    fn test_irp_inside_rept() {
        reset_expansion_counter();
        let lines = vec![
            make_line(".rept 2", 1),
            make_line("    .irp r, r1, r2", 2),
            make_line("        mov64 \\r, 0x1", 3),
            make_line("    .endr", 4),
            make_line(".endr", 5),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(
            result,
            vec![
                "        mov64 r1, 0x1",
                "        mov64 r2, 0x1",
                "        mov64 r1, 0x1",
                "        mov64 r2, 0x1",
            ]
        );
    }

    #[test]
    fn test_rept_count_from_macro_param() {
        reset_expansion_counter();
        let lines = vec![
            make_line(".macro TEST num", 1),
            make_line("    .rept \\num", 2),
            make_line("        mov64 r1, 0x123", 3),
            make_line("    .endr", 4),
            make_line(".endm", 5),
            make_line("TEST 3", 7),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(
            result,
            vec![
                "        mov64 r1, 0x123",
                "        mov64 r1, 0x123",
                "        mov64 r1, 0x123",
            ]
        );
    }

    #[test]
    fn test_irp_values_from_macro_vararg() {
        reset_expansion_counter();
        let lines = vec![
            make_line(".macro LOAD regs:vararg", 1),
            make_line("    .irp r, \\regs", 2),
            make_line("        mov64 \\r, 0x1", 3),
            make_line("    .endr", 4),
            make_line(".endm", 5),
            make_line("LOAD r1, r2, r3", 7),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(
            result,
            vec![
                "        mov64 r1, 0x1",
                "        mov64 r2, 0x1",
                "        mov64 r3, 0x1",
            ]
        );
    }

    #[test]
    fn test_unclosed_rept() {
        let lines = vec![make_line(".rept 3", 1), make_line("    nop", 2)];

        let result = expand_macros(lines);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(
            &errors[0].error,
            CompileError::UnclosedRept { .. }
        ));
        assert!(errors[0].origin.is_some());
    }

    #[test]
    fn test_full_example_from_spec() {
        reset_expansion_counter();
        let lines = vec![
            make_line(".macro DEF_STR name, text", 1),
            make_line("\\name:", 2),
            make_line("    .ascii \\text", 3),
            make_line("\\name\\()_end:", 4),
            make_line(".endm", 5),
            make_line("", 6),
            make_line(".macro RETURN_ERR code, msg", 7),
            make_line("    lddw r0, \\code", 8),
            make_line("    lddw r1, \\msg", 9),
            make_line("    lddw r2, \\msg\\()_end - \\msg", 10),
            make_line("    call sol_log_", 11),
            make_line("    exit", 12),
            make_line(".endm", 13),
            make_line("", 14),
            make_line("DEF_STR e1, \"error\"", 15),
            make_line("", 16),
            make_line("RETURN_ERR 1, e1", 17),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(
            result,
            vec![
                "", // blank line between definitions
                "", // blank line after definitions
                "e1:",
                "    .ascii \"error\"",
                "e1_end:",
                "", // blank line
                "    lddw r0, 1",
                "    lddw r1, e1",
                "    lddw r2, e1_end - e1",
                "    call sol_log_",
                "    exit",
            ]
        );
    }

    #[test]
    fn test_passthrough_no_macros() {
        let lines = vec![
            make_line(".globl entrypoint", 1),
            make_line("entrypoint:", 2),
            make_line("    mov64 r1, 42", 3),
            make_line("    exit", 4),
        ];

        let result = expand_and_collect(lines);
        assert_eq!(
            result,
            vec![
                ".globl entrypoint",
                "entrypoint:",
                "    mov64 r1, 42",
                "    exit",
            ]
        );
    }
}
