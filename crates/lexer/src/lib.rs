use serde::Serialize;
use std::iter::Peekable;

use common::{Symbol, Token, TokenType, Type};

#[cfg(test)]
mod tests;

pub type LexResult = std::result::Result<Token, LexError>;

pub struct Lexer {
    stream: Peekable<StreamIter<char>>,
    pub tokens: Vec<Token>,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            stream: StreamIter::new(input).peekable(),
            tokens: vec![],
        }
    }

    // Scan all input
    pub fn scan(mut self) -> Result<Vec<Token>, LexError> {
        loop {
            let token = self.lex()?;
            if token.is_eof() {
                break;
            }
            self.tokens.push(token);
        }
        Ok(self.tokens)
    }

    // Recursively process enough characters to produce one token
    fn lex(&mut self) -> LexResult {
        use TokenType::*;

        let cur = match self.stream.next() {
            Some(cur) => cur,
            None => unreachable!("fatal: can't lex nothing"),
        };

        // Inject a semicolon if certain tokens occur at the end of the line or
        // EOF. If EOF, make sure the context is right.
        if cur.value == '\n' && self.should_add_semicolon() {
            return Ok(Token::new(Semicolon(true), cur.line, cur.column));
        } else if cur.is_eof() {
            if self.should_add_semicolon() {
                let semi = match self.tokens.last() {
                    Some(t) => Token::new(Semicolon(true), t.line, t.column + 1),
                    None => Token::default(),
                };
                self.tokens.push(semi);
            }
            return Ok(Token::new(Eof, cur.line, cur.column));
        }

        // Ignore whitespace
        if cur.value.is_ascii_whitespace() {
            while let Some(c) = self.stream.peek() {
                if !c.value.is_ascii_whitespace() {
                    return self.lex();
                } else if c.is_eof() {
                    break;
                }
                self.stream.next();
            }
            return self.lex(); // Eat trailing newline
        }

        // Single line comments
        if cur == '/' && matches!(self.stream.peek(), Some(c) if *c == '/') {
            while let Some(c) = self.stream.next() {
                if c == '\n' {
                    return self.lex();
                } else if c.is_eof() {
                    break;
                }
            }
            return self.lex(); // Eat trailing comment
        }

        // if cur == '[' {
        //     let next = self.lex()?;
        //     match next {
        //         Token {
        //             tt: VarType(ty),
        //             line,
        //             column,
        //         } => {
        //             if self.stream.next().unwrap() == ']' {
        //                 return Ok(Token::new(VarType(Type::Array(ty.as_primative())), line, column))
        //             } else {
        //                 return Err(LexError::from(("bad thing2".to_owned(), cur)));
        //             }
        //         }
        //         _ => return Err(LexError::from(("bad thing".to_owned(), cur))),
        //     };
        // }

        // Keywords, types, and identifiers
        if cur.value.is_ascii_alphabetic() {
            let mut identifier = String::from(cur.value);
            while let Some(c) = self.stream.peek() {
                if c.value.is_ascii_alphanumeric() || *c == '_' {
                    identifier.push(c.value);
                    self.stream.next();
                } else {
                    break;
                }
            }

            let tt = match identifier.as_str() {
                "fn" => Fn,
                "let" => Let,
                "for" => For,
                "if" => If,
                "else" => Else,
                "extern" => Extern,
                "true" => Bool(true),
                "false" => Bool(false),
                "int8" => VarType(Type::Int8),
                "int16" => VarType(Type::Int16),
                "int32" => VarType(Type::Int32),
                "int64" => VarType(Type::Int64),
                "uint8" => VarType(Type::UInt8),
                "uint16" => VarType(Type::UInt16),
                "uint32" => VarType(Type::UInt32),
                "uint64" => VarType(Type::UInt64),
                "float" => VarType(Type::Float),
                "double" => VarType(Type::Double),
                "bool" => VarType(Type::Bool),
                "char" => VarType(Type::Char),
                // TODO: don't hardcode these
                "int" => VarType(Type::Int32),
                "uint" => VarType(Type::UInt32),
                _ => Ident(identifier),
            };

            return Ok(Token::new(tt, cur.line, cur.column));
        }

        // Literal numbers
        if cur.value.is_ascii_digit() {
            let mut n = String::from(cur.value);
            while let Some(c) = self.stream.peek() {
                if c.value.is_ascii_alphanumeric() || *c == '.' {
                    n.push(c.value);
                    self.stream.next();
                } else {
                    break;
                }
            }

            return Ok(Token::new(Num(n), cur.line, cur.column));
        }

        // Literal char
        if cur == '\'' {
            let mut ch = String::new();
            let next = self
                .stream
                .next()
                .unwrap_or_else(|| unreachable!("fatal: lexed None when looking for char"));

            match next.value {
                // Control characters
                '\\' => {
                    if let Some(next) = self.stream.next() {
                        match next.value {
                            'n' => ch = String::from("\n"),
                            't' => ch = String::from("\t"),
                            '\'' => ch = String::from("'"),
                            c => {
                                return Err(LexError::from((
                                    format!("Invalid character control sequence: `\\{}`", c),
                                    next,
                                )))
                            }
                        }
                    }
                }
                // EOF
                '\0' => {
                    return Err(LexError::from((
                        "Unterminated character literal. Expecting `'`, got `EOF`".to_string(),
                        cur,
                    )));
                }
                '\'' => {
                    return Err(LexError::from((
                        "Character literal can't be empty".to_string(),
                        cur,
                    )))
                }

                // Everything else
                c => ch = String::from(c),
            }

            // Check for closing '\''
            let last = self
                .stream
                .next()
                .unwrap_or_else(|| unreachable!("fatal: lexed None when looking for `'`"));
            match last.value {
                '\'' => (),
                '\0' | '\n' => {
                    return Err(LexError::from((
                        "Unterminated character literal. Expecting `'`".to_string(),
                        last,
                    )));
                }
                _ => {
                    return Err(LexError::from((
                        format!("Invalid character sequence: `'{}{}'`", ch, last.value),
                        last,
                    )));
                }
            }

            return Ok(Token::new(Char(ch), cur.line, cur.column));
        }

        // Multi-character operators
        if let Some(next) = self.stream.peek() {
            match cur.value {
                '=' if next == &'=' => {
                    self.stream.next();
                    return Ok(Token::new(Op(Symbol::Eq), cur.line, cur.column));
                }
                '&' if next == &'&' => {
                    self.stream.next();
                    return Ok(Token::new(Op(Symbol::And), cur.line, cur.column));
                }
                '|' if next == &'|' => {
                    self.stream.next();
                    return Ok(Token::new(Op(Symbol::Or), cur.line, cur.column));
                }
                '-' if next == &'>' => {
                    self.stream.next();
                    return Ok(Token::new(Op(Symbol::RetType), cur.line, cur.column));
                }
                _ => (),
            }
        }

        // Everything else
        let tt = match cur.value {
            '+' => Op(Symbol::Add),
            '-' => Op(Symbol::Sub),
            '*' => Op(Symbol::Mul),
            '/' => Op(Symbol::Div),
            '^' => Op(Symbol::Pow),
            '>' => Op(Symbol::Gt),
            '<' => Op(Symbol::Lt),
            '!' => Op(Symbol::Not),
            '=' => Op(Symbol::Assign),
            '}' => CloseBrace,
            ']' => CloseBracket,
            ')' => CloseParen,
            ':' => Colon,
            ',' => Comma,
            '{' => OpenBrace,
            '[' => OpenBracket,
            '(' => OpenParen,
            ';' => Semicolon(false),
            c => {
                return Err(LexError::from((format!("Unknown character: {}", c), cur)));
            }
        };

        Ok(Token::new(tt, cur.line, cur.column))
    }

    // Add a semicolon for these tokens
    fn should_add_semicolon(&self) -> bool {
        use TokenType::*;

        if let Some(t) = self.tokens.last() {
            matches!(
                t.tt,
                Bool(_) | Char(_) | CloseBrace | CloseParen | CloseBracket | Ident(_) | Num(_) | VarType(_)
            )
        } else {
            false
        }
    }
}

// Provides additional context for each source character
#[derive(Debug, Clone, Copy, PartialEq)]
struct ContextElement<T> {
    value: T,
    line: usize,
    column: usize,
}

impl<T> ContextElement<T> {
    fn new(value: T, line: usize, column: usize) -> Self {
        ContextElement {
            value,
            line: line + 1,
            column: column + 1,
        }
    }
}

impl ContextElement<char> {
    // Will cause lexing to stop if there's a null byte in the file
    fn is_eof(&self) -> bool {
        self.value == 0 as char
    }
}

impl PartialEq<char> for ContextElement<char> {
    fn eq(&self, other: &char) -> bool {
        self.value == *other
    }
}

// Iterator for the source character stream. Supports line and column context.
struct StreamIter<T> {
    lines: Vec<Vec<T>>,
    line: usize,
    column: usize,
}

impl StreamIter<char> {
    fn new(input: &str) -> Self {
        StreamIter {
            lines: input
                .split_inclusive('\n') // can't use .lines() or we lose '\n'
                .map(|l| l.chars().collect())
                .collect(),
            line: 0,
            column: 0,
        }
    }
}

impl Iterator for StreamIter<char> {
    type Item = ContextElement<char>;

    fn next(&mut self) -> Option<Self::Item> {
        let opt = self.lines.get(self.line);
        let line = match opt {
            Some(l) => l,
            None => return Some(ContextElement::new(0 as char, self.line, self.column - 1)),
        };
        let cc = line
            .get(self.column)
            .map(|c| ContextElement::new(*c, self.line, self.column))
            .or_else(|| {
                self.line += 1;
                self.column = 0;
                self.lines.get(self.line).and_then(|line| {
                    line.get(self.column)
                        .map(|c| ContextElement::new(*c, self.line, self.column))
                })
            });
        self.column += 1;
        cc.or_else(|| Some(ContextElement::new(0 as char, self.line, self.column - 1)))
    }
}

#[derive(Debug, PartialEq, Serialize)]
pub struct LexError {
    message: String,
    line: usize,
    column: usize,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Lexing error: {} at {}:{}",
            self.message, self.line, self.column
        )
    }
}

impl std::error::Error for LexError {}

impl<T> From<(String, ContextElement<T>)> for LexError {
    fn from((msg, cp): (String, ContextElement<T>)) -> Self {
        LexError {
            message: msg,
            line: cp.line,
            column: cp.column,
        }
    }
}
