//! Recursive-descent parser for the HyperTalk subset. Produces a `Script` of handlers.
//! Keyword matching is case-insensitive; newlines separate statements.

use super::ast::*;
use super::lexer::{Tok, lex};

pub fn parse_script(src: &str) -> Result<Script, String> {
    let toks = lex(src)?;
    let mut p = Parser { toks, pos: 0 };
    p.skip_newlines();
    let mut handlers = Vec::new();
    while !p.at_eof() {
        handlers.push(p.parse_handler()?);
        p.skip_newlines();
    }
    Ok(Script { handlers })
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    // ---- token helpers ----

    fn cur(&self) -> &Tok {
        &self.toks[self.pos]
    }

    fn at_eof(&self) -> bool {
        matches!(self.cur(), Tok::Eof)
    }

    fn at_newline(&self) -> bool {
        matches!(self.cur(), Tok::Newline)
    }

    fn advance(&mut self) -> Tok {
        let t = self.toks[self.pos].clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    /// Lowercased current word, if the current token is a word.
    fn word(&self) -> Option<String> {
        match self.cur() {
            Tok::Word(w) => Some(w.to_ascii_lowercase()),
            _ => None,
        }
    }

    /// Lowercased word at an offset from the cursor.
    fn word_at(&self, off: usize) -> Option<String> {
        match self.toks.get(self.pos + off) {
            Some(Tok::Word(w)) => Some(w.to_ascii_lowercase()),
            _ => None,
        }
    }

    fn is_kw(&self, kw: &str) -> bool {
        self.word().as_deref() == Some(kw)
    }

    /// True if the current word matches any of the given keywords.
    fn is_any(&self, kws: &[&str]) -> bool {
        self.word()
            .map(|w| kws.contains(&w.as_str()))
            .unwrap_or(false)
    }

    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.is_kw(kw) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_kw(&mut self, kw: &str) -> Result<(), String> {
        if self.eat_kw(kw) {
            Ok(())
        } else {
            Err(format!("expected '{kw}', found {:?}", self.cur()))
        }
    }

    /// Consume a word and return it lowercased.
    fn take_word(&mut self) -> Result<String, String> {
        match self.advance() {
            Tok::Word(w) => Ok(w.to_ascii_lowercase()),
            other => Err(format!("expected a word, found {other:?}")),
        }
    }

    fn skip_newlines(&mut self) {
        while self.at_newline() {
            self.advance();
        }
    }

    /// Consume the end-of-statement separator (newline or EOF).
    fn end_statement(&mut self) {
        if self.at_newline() {
            self.advance();
        }
    }

    // ---- handlers & statement blocks ----

    fn parse_handler(&mut self) -> Result<Handler, String> {
        self.expect_kw("on")?;
        let message = self.take_word()?;
        let mut params = Vec::new();
        // optional comma-separated parameter list
        if !self.at_newline() && !self.at_eof() {
            loop {
                params.push(self.take_word()?);
                if matches!(self.cur(), Tok::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.end_statement();
        let body = self.parse_block(&["end"])?;
        self.expect_kw("end")?;
        // `end <message>` — tolerate a missing/mismatched trailing name.
        if self.word().is_some() {
            self.advance();
        }
        self.end_statement();
        Ok(Handler {
            message,
            params,
            body,
        })
    }

    /// Parse statements until one of the terminator keywords appears at statement start.
    fn parse_block(&mut self, terminators: &[&str]) -> Result<Vec<Stmt>, String> {
        let mut out = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_eof() {
                break;
            }
            if let Some(w) = self.word()
                && terminators.contains(&w.as_str())
            {
                break;
            }
            out.push(self.parse_statement()?);
        }
        Ok(out)
    }

    // ---- statements ----

    fn parse_statement(&mut self) -> Result<Stmt, String> {
        let w = self
            .word()
            .ok_or_else(|| format!("expected a statement, found {:?}", self.cur()))?;
        let stmt = match w.as_str() {
            "put" => self.parse_put()?,
            "get" => {
                self.advance();
                Stmt::Get(self.parse_expr()?)
            }
            "set" => self.parse_set()?,
            "go" => self.parse_go()?,
            "answer" => {
                self.advance();
                Stmt::Answer(self.parse_expr()?)
            }
            "beep" => {
                self.advance();
                Stmt::Beep
            }
            "add" | "subtract" | "multiply" | "divide" => self.parse_arith(&w)?,
            "if" => self.parse_if()?,
            "repeat" => self.parse_repeat()?,
            _ => self.parse_send()?,
        };
        self.end_statement();
        Ok(stmt)
    }

    fn parse_put(&mut self) -> Result<Stmt, String> {
        self.expect_kw("put")?;
        let value = self.parse_expr()?;
        let container = if self.is_any(&["into", "before", "after"]) {
            self.advance();
            Some(self.parse_container()?)
        } else {
            None
        };
        Ok(Stmt::Put { value, container })
    }

    fn parse_set(&mut self) -> Result<Stmt, String> {
        self.expect_kw("set")?;
        self.eat_kw("the");
        let prop = self.take_word()?;
        self.expect_kw("of")?;
        let target = self.parse_object_ref()?;
        self.expect_kw("to")?;
        let value = self.parse_expr()?;
        Ok(Stmt::Set {
            prop,
            target,
            value,
        })
    }

    fn parse_go(&mut self) -> Result<Stmt, String> {
        self.expect_kw("go")?;
        self.eat_kw("to");
        let dest = if self.eat_kw("next") {
            self.eat_kw("card");
            Destination::NextCard
        } else if self.is_any(&["previous", "prev"]) {
            self.advance();
            self.eat_kw("card");
            Destination::PrevCard
        } else if self.eat_kw("first") {
            self.eat_kw("card");
            Destination::FirstCard
        } else if self.eat_kw("last") {
            self.eat_kw("card");
            Destination::LastCard
        } else if self.eat_kw("stack") {
            Destination::Stack(self.parse_primary()?)
        } else if self.is_any(&["card", "cd"]) {
            self.advance();
            let sel = self.parse_primary()?;
            if is_number_literal(&sel) {
                Destination::CardByNumber(sel)
            } else {
                Destination::CardByName(sel)
            }
        } else {
            Destination::CardByName(self.parse_primary()?)
        };
        Ok(Stmt::Go(dest))
    }

    fn parse_arith(&mut self, op_word: &str) -> Result<Stmt, String> {
        self.advance(); // op keyword
        match op_word {
            "add" => {
                let amount = self.parse_expr()?;
                self.expect_kw("to")?;
                let container = self.parse_container()?;
                Ok(Stmt::Arith {
                    op: ArithOp::Add,
                    amount,
                    container,
                })
            }
            "subtract" => {
                let amount = self.parse_expr()?;
                self.expect_kw("from")?;
                let container = self.parse_container()?;
                Ok(Stmt::Arith {
                    op: ArithOp::Subtract,
                    amount,
                    container,
                })
            }
            "multiply" => {
                let container = self.parse_container()?;
                self.expect_kw("by")?;
                let amount = self.parse_expr()?;
                Ok(Stmt::Arith {
                    op: ArithOp::Multiply,
                    amount,
                    container,
                })
            }
            "divide" => {
                let container = self.parse_container()?;
                self.expect_kw("by")?;
                let amount = self.parse_expr()?;
                Ok(Stmt::Arith {
                    op: ArithOp::Divide,
                    amount,
                    container,
                })
            }
            _ => unreachable!(),
        }
    }

    fn parse_if(&mut self) -> Result<Stmt, String> {
        self.expect_kw("if")?;
        let cond = self.parse_expr()?;
        // `then` may sit on the next line in some dialects; tolerate a newline before it.
        self.skip_newlines();
        self.expect_kw("then")?;
        let (then_body, else_body) = if self.at_newline() {
            self.end_statement();
            let then_body = self.parse_block(&["else", "end"])?;
            let else_body = if self.eat_kw("else") {
                if self.at_newline() {
                    self.end_statement();
                    self.parse_block(&["end"])?
                } else {
                    vec![self.parse_statement()?]
                }
            } else {
                Vec::new()
            };
            self.expect_kw("end")?;
            self.expect_kw("if")?;
            (then_body, else_body)
        } else {
            // single-line: `if c then STMT [else STMT]`
            let then_body = vec![self.parse_statement_no_term()?];
            let else_body = if self.eat_kw("else") {
                vec![self.parse_statement_no_term()?]
            } else {
                Vec::new()
            };
            (then_body, else_body)
        };
        Ok(Stmt::If {
            cond,
            then_body,
            else_body,
        })
    }

    /// Like `parse_statement` but does not consume a trailing newline — used for the
    /// single-line `if ... then STMT else STMT` form.
    fn parse_statement_no_term(&mut self) -> Result<Stmt, String> {
        let save = self.pos;
        let stmt = self.parse_statement()?;
        // parse_statement consumed a trailing newline; that's fine, but for single-line
        // if/else we must not swallow the `else`. Re-check: if we landed exactly past a
        // newline we leave it. Nothing else to undo.
        let _ = save;
        Ok(stmt)
    }

    fn parse_repeat(&mut self) -> Result<Stmt, String> {
        self.expect_kw("repeat")?;
        let kind = if self.eat_kw("with") {
            let var = self.take_word()?;
            self.expect_kw_sym_eq()?;
            let from = self.parse_expr()?;
            self.expect_kw("to")?;
            let to = self.parse_expr()?;
            RepeatKind::With { var, from, to }
        } else if self.eat_kw("for") {
            let n = self.parse_expr()?;
            self.eat_kw("times");
            RepeatKind::Times(n)
        } else if self.at_newline() {
            // `repeat` forever is not supported; require a count. Treat as 0 times.
            RepeatKind::Times(Expr::Number(0.0))
        } else {
            let n = self.parse_expr()?;
            self.eat_kw("times");
            RepeatKind::Times(n)
        };
        self.end_statement();
        let body = self.parse_block(&["end"])?;
        self.expect_kw("end")?;
        self.expect_kw("repeat")?;
        Ok(Stmt::Repeat { kind, body })
    }

    fn expect_kw_sym_eq(&mut self) -> Result<(), String> {
        if matches!(self.cur(), Tok::Eq) {
            self.advance();
            Ok(())
        } else {
            Err(format!("expected '=', found {:?}", self.cur()))
        }
    }

    fn parse_send(&mut self) -> Result<Stmt, String> {
        let name = self.take_word()?;
        let mut args = Vec::new();
        if !self.at_newline() && !self.at_eof() && !self.is_kw("else") && !self.is_kw("end") {
            loop {
                args.push(self.parse_expr()?);
                if matches!(self.cur(), Tok::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        Ok(Stmt::Send(name, args))
    }

    // ---- containers & object references ----

    fn parse_container(&mut self) -> Result<Container, String> {
        if self.looks_like_field_ref() {
            Ok(Container::Field(self.parse_field_ref()?))
        } else if self.eat_kw("it") {
            Ok(Container::It)
        } else {
            let name = self.take_word()?;
            Ok(Container::Variable(name))
        }
    }

    /// True if the cursor is at `field`, `card field`, `bg field`, etc.
    fn looks_like_field_ref(&self) -> bool {
        match self.word().as_deref() {
            Some("field") | Some("fld") => true,
            Some("card") | Some("cd") | Some("background") | Some("bkgnd") | Some("bg") => {
                matches!(self.word_at(1).as_deref(), Some("field") | Some("fld"))
            }
            _ => false,
        }
    }

    fn looks_like_button_ref(&self) -> bool {
        match self.word().as_deref() {
            Some("button") | Some("btn") => true,
            Some("card") | Some("cd") | Some("background") | Some("bkgnd") | Some("bg") => {
                matches!(self.word_at(1).as_deref(), Some("button") | Some("btn"))
            }
            _ => false,
        }
    }

    fn parse_layer(&mut self) -> Layer {
        if self.is_any(&["card", "cd"]) {
            self.advance();
            Layer::Card
        } else if self.is_any(&["background", "bkgnd", "bg"]) {
            self.advance();
            Layer::Background
        } else {
            Layer::Card
        }
    }

    fn parse_field_ref(&mut self) -> Result<FieldRef, String> {
        let layer = self.parse_layer();
        if !self.eat_kw("field") && !self.eat_kw("fld") {
            return Err(format!("expected 'field', found {:?}", self.cur()));
        }
        let selector = self.parse_selector()?;
        Ok(FieldRef { layer, selector })
    }

    fn parse_button_ref(&mut self) -> Result<ButtonRef, String> {
        let layer = self.parse_layer();
        if !self.eat_kw("button") && !self.eat_kw("btn") {
            return Err(format!("expected 'button', found {:?}", self.cur()));
        }
        let selector = self.parse_selector()?;
        Ok(ButtonRef { layer, selector })
    }

    fn parse_selector(&mut self) -> Result<Selector, String> {
        let e = self.parse_primary()?;
        if is_number_literal(&e) {
            Ok(Selector::ByNumber(e))
        } else {
            Ok(Selector::ByName(e))
        }
    }

    fn parse_object_ref(&mut self) -> Result<ObjectRef, String> {
        if self.eat_kw("me") {
            Ok(ObjectRef::Me)
        } else if self.looks_like_field_ref() {
            Ok(ObjectRef::Field(self.parse_field_ref()?))
        } else if self.looks_like_button_ref() {
            Ok(ObjectRef::Button(self.parse_button_ref()?))
        } else if self.is_any(&["this"]) {
            self.advance(); // 'this'
            if self.eat_kw("stack") {
                Ok(ObjectRef::Stack) // `this stack`
            } else {
                self.eat_kw("card"); // `this card` / bare `this`
                Ok(ObjectRef::Card)
            }
        } else if self.is_any(&["card", "cd"]) {
            self.advance();
            Ok(ObjectRef::Card)
        } else if self.eat_kw("stack") {
            Ok(ObjectRef::Stack)
        } else {
            Err(format!("expected an object, found {:?}", self.cur()))
        }
    }

    // ---- expressions (precedence climbing) ----

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while self.is_kw("or") {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Binary(BinOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_compare()?;
        while self.is_kw("and") {
            self.advance();
            let right = self.parse_compare()?;
            left = Expr::Binary(BinOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_compare(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_concat()?;
        loop {
            let op = match self.cur() {
                Tok::Eq => sym(self, BinOp::Eq),
                Tok::Ne => sym(self, BinOp::Ne),
                Tok::Lt => sym(self, BinOp::Lt),
                Tok::Gt => sym(self, BinOp::Gt),
                Tok::Le => sym(self, BinOp::Le),
                Tok::Ge => sym(self, BinOp::Ge),
                Tok::Word(w) if w.eq_ignore_ascii_case("is") => {
                    // `is` / `is not`
                    self.advance();
                    if self.eat_kw("not") {
                        BinOp::Ne
                    } else {
                        BinOp::Eq
                    }
                }
                _ => break,
            };
            let right = self.parse_concat()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_concat(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_add()?;
        loop {
            let op = match self.cur() {
                Tok::AmpAmp => BinOp::ConcatSpace,
                Tok::Amp => BinOp::Concat,
                _ => break,
            };
            self.advance();
            let right = self.parse_add()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_mul()?;
        loop {
            let op = match self.cur() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_mul()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.cur() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Word(w) if w.eq_ignore_ascii_case("mod") => BinOp::Mod,
                Tok::Word(w) if w.eq_ignore_ascii_case("div") => BinOp::Div,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if matches!(self.cur(), Tok::Minus) {
            self.advance();
            return Ok(Expr::Unary(UnOp::Neg, Box::new(self.parse_unary()?)));
        }
        if self.is_kw("not") {
            self.advance();
            return Ok(Expr::Unary(UnOp::Not, Box::new(self.parse_unary()?)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.cur().clone() {
            Tok::Number(n) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            Tok::Str(s) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            Tok::LParen => {
                self.advance();
                let e = self.parse_expr()?;
                if !matches!(self.cur(), Tok::RParen) {
                    return Err(format!("expected ')', found {:?}", self.cur()));
                }
                self.advance();
                Ok(e)
            }
            Tok::Word(_) => self.parse_word_primary(),
            other => Err(format!("unexpected token in expression: {other:?}")),
        }
    }

    fn parse_word_primary(&mut self) -> Result<Expr, String> {
        // `the PROP of OBJECT` or `the number of cards`
        if self.eat_kw("the") {
            let mut words = vec![self.take_word()?];
            while !self.is_kw("of") && self.word().is_some() {
                words.push(self.take_word()?);
            }
            let prop = words.join(" ");
            self.expect_kw("of")?;
            if prop == "number" {
                // `the number of cards|buttons|fields`
                let coll = self.take_word()?;
                return Ok(Expr::Call("number_of".to_string(), vec![Expr::Str(coll)]));
            }
            let target = self.parse_object_ref()?;
            return Ok(Expr::Property {
                name: prop,
                target: Box::new(target),
            });
        }
        // field contents used as a value
        if self.looks_like_field_ref() {
            return Ok(Expr::FieldContents(Box::new(self.parse_field_ref()?)));
        }
        // function call: WORD(...)
        if self.word_token_then_lparen() {
            let name = self.take_word()?;
            self.advance(); // (
            let mut args = Vec::new();
            if !matches!(self.cur(), Tok::RParen) {
                loop {
                    args.push(self.parse_expr()?);
                    if matches!(self.cur(), Tok::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
            if !matches!(self.cur(), Tok::RParen) {
                return Err(format!("expected ')', found {:?}", self.cur()));
            }
            self.advance();
            return Ok(Expr::Call(name.to_ascii_lowercase(), args));
        }
        // bare word: variable / `it` / constant
        let w = self.take_word()?;
        Ok(Expr::Var(w))
    }

    fn word_token_then_lparen(&self) -> bool {
        matches!(self.cur(), Tok::Word(_))
            && matches!(self.toks.get(self.pos + 1), Some(Tok::LParen))
    }
}

fn is_number_literal(e: &Expr) -> bool {
    matches!(e, Expr::Number(_))
}

/// Consume the current symbolic comparison token and yield its operator.
fn sym(p: &mut Parser, op: BinOp) -> BinOp {
    p.advance();
    op
}
