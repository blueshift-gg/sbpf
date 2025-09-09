use crate::define_compile_errors;
use std::ops::Range;

// labels could be overridden by passing a valid custom_label in the error variant
// if not provided, the label will use default messages from below
define_compile_errors! {
    // Lexical errors
    InvalidNumber {
        error = "Invalid number '{number}'",
        label = "Invalid number",
        fields = { number: String, span: Range<usize> }
    },
    InvalidRegister {
        error = "Invalid register '{register}'",
        label = "Invalid register",
        fields = { register: String, span: Range<usize> }
    },
    UnexpectedCharacter {
        error = "Unexpected character '{character}'",
        label = "Unexpected character",
        fields = { character: char, span: Range<usize> }
    },
    UnterminatedStringLiteral {
        error = "Unterminated string literal",
        label = "Unterminated string literal",
        fields = { span: Range<usize> }
    },
    // Syntactic errors
    InvalidGlobalDecl {
        error = "Invalid global declaration",
        label = "Expected <identifier> for entry label",
        fields = { span: Range<usize> }
    },
    InvalidExternDecl {
        error = "Invalid extern declaration",
        label = "Invalid extern declaration",
        fields = { span: Range<usize> }
    },
    InvalidRodataDecl {
        error = "Invalid rodata declaration",
        label = "Invalid rodata declaration",
        fields = { span: Range<usize> }
    },
    InvalidEquDecl {
        error = "Invalid equ declaration",
        label = "Invalid equ declaration",
        fields = { span: Range<usize> }
    },
    InvalidDirective {
        error = "Invalid directive '{directive}'",
        label = "Invalid directive",
        fields = { directive: String, span: Range<usize> }
    },
    InvalidInstruction {
        error = "Invalid '{instruction}' instruction",
        label = "Invalid instruction",
        fields = { instruction: String, span: Range<usize> }
    },
    UnexpectedToken {
        error = "Unexpected token '{token}'",
        label = "Unexpected token",
        fields = { token: String, span: Range<usize> }
    },
    UnmatchedParen {
        error = "Unmatched parenthesis",
        label = "Unmatched parenthesis",
        fields = { span: Range<usize> }
    },
    OutOfRangeLiteral {
        error = "Out of range literal'",
        label = "Out of range literal",
        fields = { span: Range<usize> }
    },
    InvalidRODataDirective {
        error = "Invalid rodata directive",
        label = "Invalid rodata directive",
        fields = { span: Range<usize> }
    },
    // Semantic errors
    UndefinedLabel {
        error = "Undefined label '{label}'",
        label = "Undefined label",
        fields = { label: String, span: Range<usize> }
    },
    DuplicateLabel {
        error = "Duplicate label '{label}'",
        label = "Label redefined",
        fields = { label: String, span: Range<usize>, original_span: Range<usize> }
    },
}