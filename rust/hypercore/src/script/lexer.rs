//! A small line-oriented tokenizer for the HyperTalk subset. HyperTalk statements are
//! separated by newlines, so newlines are significant tokens. `--` begins a comment to
//! end of line.

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Word(String),
    Number(f64),
    Str(String),
    Newline,
    // operators / punctuation
    Amp,    // &
    AmpAmp, // &&
    Plus,   // +
    Minus,  // -
    Star,   // *
    Slash,  // /
    Eq,     // =
    Ne,     // <> / ≠
    Lt,     // <
    Gt,     // >
    Le,     // <=
    Ge,     // >=
    LParen, // (
    RParen, // )
    Comma,  // ,
    Eof,
}

pub fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\r' => {
                i += 1;
            }
            '\n' => {
                // Collapse runs of blank lines into a single Newline token.
                if out.last() != Some(&Tok::Newline) && !out.is_empty() {
                    out.push(Tok::Newline);
                }
                i += 1;
            }
            '-' if i + 1 < chars.len() && chars[i + 1] == '-' => {
                // line comment
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '"' => {
                i += 1;
                let mut s = String::new();
                while i < chars.len() && chars[i] != '"' {
                    s.push(chars[i]);
                    i += 1;
                }
                if i >= chars.len() {
                    return Err("unterminated string literal".to_string());
                }
                i += 1; // closing quote
                out.push(Tok::Str(s));
            }
            '&' => {
                if i + 1 < chars.len() && chars[i + 1] == '&' {
                    out.push(Tok::AmpAmp);
                    i += 2;
                } else {
                    out.push(Tok::Amp);
                    i += 1;
                }
            }
            '+' => {
                out.push(Tok::Plus);
                i += 1;
            }
            '-' => {
                out.push(Tok::Minus);
                i += 1;
            }
            '*' => {
                out.push(Tok::Star);
                i += 1;
            }
            '/' => {
                out.push(Tok::Slash);
                i += 1;
            }
            '=' => {
                out.push(Tok::Eq);
                i += 1;
            }
            '≠' => {
                out.push(Tok::Ne);
                i += 1;
            }
            '≤' => {
                out.push(Tok::Le);
                i += 1;
            }
            '≥' => {
                out.push(Tok::Ge);
                i += 1;
            }
            '<' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    out.push(Tok::Le);
                    i += 2;
                } else if i + 1 < chars.len() && chars[i + 1] == '>' {
                    out.push(Tok::Ne);
                    i += 2;
                } else {
                    out.push(Tok::Lt);
                    i += 1;
                }
            }
            '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    out.push(Tok::Ge);
                    i += 2;
                } else {
                    out.push(Tok::Gt);
                    i += 1;
                }
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            c if c.is_ascii_digit()
                || (c == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) =>
            {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                let n: f64 = s
                    .parse()
                    .map_err(|_| format!("invalid number literal: {s}"))?;
                out.push(Tok::Number(n));
            }
            c if is_word_start(c) => {
                let start = i;
                while i < chars.len() && is_word_part(chars[i]) {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                out.push(Tok::Word(s));
            }
            other => {
                return Err(format!("unexpected character '{other}'"));
            }
        }
    }
    out.push(Tok::Eof);
    Ok(out)
}

fn is_word_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_word_part(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
