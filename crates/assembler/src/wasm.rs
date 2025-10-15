use crate::lexer::tokenize;
use crate::parser::Parser;
use crate::program::Program;
use serde::Serialize;
use serde_wasm_bindgen::to_value;
use std::ops::Range;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct CompileErrorInfo {
    error: String,
    line: String,
    col: String,
}

// Helper function to convert byte span to line/column numbers
fn span_to_line_col(source_code: &str, span: &Range<usize>) -> (usize, usize) {
    // Convert byte position to line number (1-based)
    let mut line = 1;
    let mut current_pos = 0;

    for (i, c) in source_code.char_indices() {
        if i >= span.start {
            break;
        }
        if c == '\n' {
            line += 1;
            current_pos = i + 1;
        }
    }

    // Calculate column number (1-based) by finding the start of the line
    let column = span.start - current_pos + 1;

    (line, column)
}

#[wasm_bindgen]
pub fn assemble(source: &str) -> Result<Vec<u8>, JsValue> {
    let tokens = match tokenize(source) {
        Ok(tokens) => tokens,
        Err(errors) => {
            let compile_errors: Vec<CompileErrorInfo> = errors
                .iter()
                .map(|e| {
                    let (line, col) = span_to_line_col(source, e.span());
                    CompileErrorInfo {
                        error: e.to_string(),
                        line: line.to_string(),
                        col: col.to_string(),
                    }
                })
                .collect();
            return Err(to_value(&compile_errors).unwrap());
        }
    };
    let mut parser = Parser::new(tokens);
    let parse_result = match parser.parse() {
        Ok(program) => program,
        Err(errors) => {
            let compile_errors: Vec<CompileErrorInfo> = errors
                .iter()
                .map(|e| {
                    let (line, col) = span_to_line_col(source, e.span());
                    CompileErrorInfo {
                        error: e.to_string(),
                        line: line.to_string(),
                        col: col.to_string(),
                    }
                })
                .collect();
            return Err(to_value(&compile_errors).unwrap());
        }
    };
    let program = Program::from_parse_result(parse_result);
    let bytecode = program.emit_bytecode();
    Ok(bytecode)
}
