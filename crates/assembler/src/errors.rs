use {crate::define_compile_errors, std::ops::Range};

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
    ParseError {
        error = "Parse error: {error}",
        label = "Parse error",
        fields = { error: String, span: Range<usize> }
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
    CrossSectionArithmetic {
        error = "Cross-section label arithmetic: '{label1}' and '{label2}' are in different sections",
        label = "Cross-section arithmetic",
        fields = { label1: String, label2: String, span: Range<usize> }
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
    BytecodeError {
        error = "Bytecode error: {error}",
        label = "Bytecode error",
        fields = { error: String, span: Range<usize> }
    },
    MissingTextDirective {
        error = "Missing text directive",
        label = "Missing text directive",
        fields = { span: Range<usize> }
    },
    // Preprocessor errors
    IncludeCycle {
        error = "Include cycle detected: '{path}'",
        label = "Include cycle",
        fields = { path: String, span: Range<usize> }
    },
    IncludeNotFound {
        error = "Include file not found: '{path}'",
        label = "File not found",
        fields = { path: String, span: Range<usize> }
    },
    IncludeReadError {
        error = "Failed to read include file '{path}': {reason}",
        label = "Read error",
        fields = { path: String, reason: String, span: Range<usize> }
    },
    UnclosedMacro {
        error = "Macro '{name}' missing .endm",
        label = "Unclosed macro definition",
        fields = { name: String, span: Range<usize> }
    },
    UnclosedRept {
        error = "Missing .endr for .rept/.irp",
        label = "Unclosed repetition block",
        fields = { span: Range<usize> }
    },
    DuplicateMacroDef {
        error = "Macro '{name}' already defined",
        label = "Duplicate macro definition",
        fields = { name: String, span: Range<usize> }
    },
    MacroArgCount {
        error = "Macro '{name}' expects {expected} argument(s), got {got}",
        label = "Wrong number of arguments",
        fields = { name: String, expected: usize, got: usize, span: Range<usize> }
    },
    UndefinedMacroParam {
        error = "Undefined macro parameter '\\{param}'",
        label = "Unknown parameter",
        fields = { param: String, span: Range<usize> }
    },
    MacroRecursionLimit {
        error = "Macro expansion depth exceeded (max {limit})",
        label = "Recursion limit exceeded",
        fields = { limit: u32, span: Range<usize> }
    },
    InvalidReptCount {
        error = "Invalid .rept count: '{value}'",
        label = "Invalid repeat count",
        fields = { value: String, span: Range<usize> }
    },
    VarargNotLast {
        error = "Vararg parameter must be last in macro '{name}'",
        label = "Vararg not last",
        fields = { name: String, span: Range<usize> }
    },
    MultipleVararg {
        error = "Multiple :vararg parameters in macro '{name}'",
        label = "Multiple vararg parameters",
        fields = { name: String, span: Range<usize> }
    },
}
