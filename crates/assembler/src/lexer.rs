use crate::errors::CompileError;
use sbpf_common::opcode::Opcode;
use std::{ops::Range, str::FromStr as _};

#[derive(Debug, Clone)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImmediateValue {
    Int(i64),
    Addr(i64),
}

impl std::ops::Add for ImmediateValue {
    type Output = ImmediateValue;
    fn add(self, other: Self) -> ImmediateValue {
        match (self, other) {
            (ImmediateValue::Int(a), ImmediateValue::Int(b)) => ImmediateValue::Int(a + b),
            (ImmediateValue::Addr(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a + b),
            (ImmediateValue::Int(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a + b),
            (ImmediateValue::Addr(a), ImmediateValue::Int(b)) => ImmediateValue::Addr(a + b),
        }
    }
}

impl std::ops::Sub for ImmediateValue {
    type Output = ImmediateValue;
    fn sub(self, other: Self) -> ImmediateValue {
        match (self, other) {
            (ImmediateValue::Int(a), ImmediateValue::Int(b)) => ImmediateValue::Int(a - b),
            (ImmediateValue::Addr(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a - b),
            (ImmediateValue::Int(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a - b),
            (ImmediateValue::Addr(a), ImmediateValue::Int(b)) => ImmediateValue::Addr(a - b),
        }
    }
}

impl std::ops::Mul for ImmediateValue {
    type Output = ImmediateValue;
    fn mul(self, other: Self) -> ImmediateValue {
        match (self, other) {
            (ImmediateValue::Int(a), ImmediateValue::Int(b)) => ImmediateValue::Int(a * b),
            (ImmediateValue::Addr(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a * b),
            (ImmediateValue::Int(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a * b),
            (ImmediateValue::Addr(a), ImmediateValue::Int(b)) => ImmediateValue::Addr(a * b),
        }
    }
}

impl std::ops::Div for ImmediateValue {
    type Output = ImmediateValue;
    fn div(self, other: Self) -> ImmediateValue {
        match (self, other) {
            (ImmediateValue::Int(a), ImmediateValue::Int(b)) => ImmediateValue::Int(a / b),
            (ImmediateValue::Addr(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a / b),
            (ImmediateValue::Int(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a / b),
            (ImmediateValue::Addr(a), ImmediateValue::Int(b)) => ImmediateValue::Addr(a / b),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Token {
    Directive(String, Range<usize>),
    Label(String, Range<usize>),
    Identifier(String, Range<usize>),
    Opcode(Opcode, Range<usize>),
    Register(u8, Range<usize>),
    ImmediateValue(ImmediateValue, Range<usize>),
    BinaryOp(Op, Range<usize>),
    StringLiteral(String, Range<usize>),
    VectorLiteral(Vec<ImmediateValue>, Range<usize>),

    LeftBracket(Range<usize>),
    RightBracket(Range<usize>),
    LeftParen(Range<usize>),
    RightParen(Range<usize>),
    Comma(Range<usize>),
    Colon(Range<usize>),

    Newline(Range<usize>),
}

pub fn tokenize(source: &str) -> Result<Vec<Token>, Vec<CompileError>> {
    let mut tokens = Vec::new();
    let mut errors = Vec::new();
    let mut byte_offset = 0;

    let mut paren_stack: Vec<Token> = Vec::new();

    for line in source.lines() {
        if line.is_empty() {
            byte_offset += 1;
            continue;
        }
        let mut chars = line.char_indices().peekable();
        while let Some((start_idx, c)) = chars.peek() {
            let token_start = byte_offset + start_idx;
            match c {
                c if c.is_ascii_digit() => {
                    let mut number = String::new();
                    let mut is_addr = false;
                    while let Some((_, c)) = chars.peek() {
                        if c.is_ascii_digit() {
                            number.push(chars.next().unwrap().1);
                        } else if number == "0" && *c == 'x' {
                            chars.next();
                            is_addr = true; /*  */
                            number = String::new();
                        } else if is_addr
                            && (*c == 'a'
                                || *c == 'b'
                                || *c == 'c'
                                || *c == 'd'
                                || *c == 'e'
                                || *c == 'f')
                        {
                            number.push(chars.next().unwrap().1);
                        } else {
                            break;
                        }
                    }
                    let span = token_start..token_start + number.len();
                    if is_addr {
                        if let Ok(value) = u64::from_str_radix(&number, 16) {
                            let value = value as i64;
                            tokens.push(Token::ImmediateValue(
                                ImmediateValue::Addr(value),
                                span.clone(),
                            ));
                        } else {
                            errors.push(CompileError::InvalidNumber {
                                number,
                                span: span.clone(),
                                custom_label: None,
                            });
                        }
                    } else if let Ok(value) = number.parse::<i64>() {
                        tokens.push(Token::ImmediateValue(
                            ImmediateValue::Int(value),
                            span.clone(),
                        ));
                    } else {
                        errors.push(CompileError::InvalidNumber {
                            number,
                            span: span.clone(),
                            custom_label: None,
                        });
                    }
                }

                c if c.is_ascii_alphanumeric() || *c == '_' => {
                    let mut identifier = String::new();
                    while let Some((_, c)) = chars.peek() {
                        if *c == '_' || *c == ':' || *c == '.' || c.is_ascii_alphanumeric() {
                            identifier.push(chars.next().unwrap().1);
                        } else {
                            break;
                        }
                    }
                    let span = token_start..token_start + identifier.len();
                    if identifier.ends_with(':') {
                        let label_name = identifier.trim_end_matches(':').to_string();
                        tokens.push(Token::Label(label_name, span));
                    } else if identifier.starts_with('r')
                        && identifier[1..].chars().all(|c| c.is_ascii_digit())
                    {
                        // TODO: label name can be "r"
                        if let Ok(value) = identifier[1..].parse::<u8>() {
                            tokens.push(Token::Register(value, span.clone()));
                        } else {
                            errors.push(CompileError::InvalidRegister {
                                register: identifier,
                                span: span.clone(),
                                custom_label: None,
                            });
                        }
                    } else if let Ok(opcode) = Opcode::from_str(&identifier) {
                        tokens.push(Token::Opcode(opcode, span));
                    } else {
                        tokens.push(Token::Identifier(identifier, span));
                    }
                }
                c if c.is_whitespace() => {
                    chars.next();
                }
                '+' => {
                    chars.next();
                    let span = token_start..token_start + 1;
                    tokens.push(Token::BinaryOp(Op::Add, span));
                }
                '-' => {
                    chars.next();
                    let span = token_start..token_start + 1;
                    tokens.push(Token::BinaryOp(Op::Sub, span));
                }
                '*' => {
                    chars.next();
                    let span = token_start..token_start + 1;
                    tokens.push(Token::BinaryOp(Op::Mul, span));
                }
                '.' => {
                    chars.next();
                    let directive: String = chars
                        .by_ref()
                        .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '_')
                        .map(|(_, c)| c)
                        .collect();
                    let span = token_start..token_start + directive.len() + 1;
                    tokens.push(Token::Directive(directive, span));
                }
                '"' => {
                    chars.next();
                    let mut string_literal = String::new();
                    while let Some((_, c)) = chars.peek() {
                        if *c == '"' {
                            chars.next();
                            let span = token_start..token_start + string_literal.len() + 2;
                            tokens.push(Token::StringLiteral(string_literal, span));
                            break;
                        } else if *c == '\n' {
                            errors.push(CompileError::UnterminatedStringLiteral {
                                span: token_start..token_start + 1,
                                custom_label: None,
                            });
                        }
                        string_literal.push(chars.next().unwrap().1);
                    }
                }
                '(' => {
                    chars.next();
                    let span = token_start..token_start + 1;
                    let token = Token::LeftParen(span);
                    paren_stack.push(token.clone());
                    tokens.push(token);
                }
                ')' => {
                    chars.next();
                    let span = token_start..token_start + 1;
                    paren_stack.pop();
                    tokens.push(Token::RightParen(span));
                }
                '[' => {
                    chars.next();
                    let span = token_start..token_start + 1;
                    tokens.push(Token::LeftBracket(span));
                }
                ']' => {
                    chars.next();
                    let span = token_start..token_start + 1;
                    tokens.push(Token::RightBracket(span));
                }
                ',' => {
                    chars.next();
                    let span = token_start..token_start + 1;
                    tokens.push(Token::Comma(span));
                }
                // handle comments
                '#' => {
                    chars.next();
                    break;
                }
                '/' => {
                    chars.next();
                    if let Some((_, '/')) = chars.peek() {
                        chars.next();
                        break;
                    } else {
                        chars.next();
                        let span = token_start..token_start + 1;
                        tokens.push(Token::BinaryOp(Op::Div, span));
                    }
                }
                _ => {
                    let span = token_start..token_start + 1;
                    errors.push(CompileError::UnexpectedCharacter {
                        character: *c,
                        span,
                        custom_label: None,
                    });
                    chars.next();
                }
            }
        }
        byte_offset += line.len();
        // tokens.push(Token::Newline(byte_offset..byte_offset + 1));
        byte_offset += 1;
    }

    while let Some(Token::LeftParen(span)) = paren_stack.pop() {
        errors.push(CompileError::UnmatchedParen {
            span,
            custom_label: None,
        });
    }

    if errors.is_empty() {
        Ok(tokens)
    } else {
        Err(errors)
    }
}
