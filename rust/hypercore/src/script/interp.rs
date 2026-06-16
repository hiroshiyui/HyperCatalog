//! The HyperTalk interpreter / runtime. A `Runtime` borrows the document `Stack`,
//! tracks the current card, executes handler bodies, and accumulates host effects
//! (dialogs, beeps, message-box output) for the platform host to perform.

use std::collections::HashMap;

use super::ast::*;
use super::parser::parse_script;
use super::value::Value;
use crate::model::Stack;

/// Side effects the core cannot perform itself; the host carries them out.
#[derive(Clone, Debug, PartialEq)]
pub enum HostCmd {
    /// `answer "..."` — show a modal dialog.
    Answer(String),
    /// `beep`.
    Beep,
    /// `put "..."` with no container — message-box output.
    Message(String),
}

/// Identifies the object whose script is currently running (`me`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Me {
    Button(u32),
    Field(u32),
    Card,
    Background(u32),
    Stack,
}

/// Per-handler local variable scope plus the implicit `it`.
#[derive(Default)]
pub struct Env {
    vars: HashMap<String, Value>,
    it: Value,
}

impl Env {
    fn new() -> Env {
        Env {
            vars: HashMap::new(),
            it: Value::Empty,
        }
    }
}

/// Control-flow signal threaded out of statement execution.
#[derive(Clone, Copy, PartialEq)]
enum Flow {
    Next,
    ExitRepeat,
    ExitHandler,
}

/// Where a located field/button physically lives.
enum Loc {
    CardObj(usize),
    BgObj(usize, usize),
}

pub struct Runtime<'s> {
    pub stack: &'s mut Stack,
    pub card_index: usize,
    pub host: Vec<HostCmd>,
    /// Simple deterministic PRNG state for `random()` (we avoid OS randomness).
    rng: u64,
}

impl<'s> Runtime<'s> {
    pub fn new(stack: &'s mut Stack, card_index: usize) -> Runtime<'s> {
        Runtime {
            stack,
            card_index,
            host: Vec::new(),
            rng: 0x2545_F491_4F6C_DD1D,
        }
    }

    /// Run the named message handler found in `src` (if any), with `me` as the owner.
    /// Returns Ok(true) if a matching handler ran.
    pub fn run_handler(
        &mut self,
        src: &str,
        message: &str,
        me: Me,
        args: &[Value],
    ) -> Result<bool, String> {
        let script = parse_script(src)?;
        let target = message.to_ascii_lowercase();
        let Some(handler) = script.handlers.iter().find(|h| h.message == target) else {
            return Ok(false);
        };
        let mut env = Env::new();
        for (i, p) in handler.params.iter().enumerate() {
            env.vars
                .insert(p.clone(), args.get(i).cloned().unwrap_or(Value::Empty));
        }
        self.exec_stmts(&handler.body, &mut env, me)?;
        Ok(true)
    }

    // ---- statement execution ----

    fn exec_stmts(&mut self, stmts: &[Stmt], env: &mut Env, me: Me) -> Result<Flow, String> {
        for s in stmts {
            let flow = self.exec_stmt(s, env, me)?;
            if flow != Flow::Next {
                return Ok(flow);
            }
        }
        Ok(Flow::Next)
    }

    fn exec_stmt(&mut self, stmt: &Stmt, env: &mut Env, me: Me) -> Result<Flow, String> {
        match stmt {
            Stmt::Put { value, container } => {
                let v = self.eval(value, env, me)?;
                match container {
                    Some(c) => self.container_set(c, v, env, me)?,
                    None => self.host.push(HostCmd::Message(v.as_text())),
                }
            }
            Stmt::Get(e) => {
                let v = self.eval(e, env, me)?;
                env.it = v;
            }
            Stmt::Set {
                prop,
                target,
                value,
            } => {
                let v = self.eval(value, env, me)?;
                self.set_property(prop, target, v, env, me)?;
            }
            Stmt::Go(dest) => self.exec_go(dest, env, me)?,
            Stmt::Answer(e) => {
                let v = self.eval(e, env, me)?;
                self.host.push(HostCmd::Answer(v.as_text()));
            }
            Stmt::Beep => self.host.push(HostCmd::Beep),
            Stmt::Arith {
                op,
                amount,
                container,
            } => {
                let amt = self
                    .eval(amount, env, me)?
                    .as_number()
                    .ok_or("arithmetic on non-number")?;
                let cur = self
                    .container_get(container, env, me)?
                    .as_number()
                    .ok_or("arithmetic on non-number container")?;
                let result = match op {
                    ArithOp::Add => cur + amt,
                    ArithOp::Subtract => cur - amt,
                    ArithOp::Multiply => cur * amt,
                    ArithOp::Divide => cur / amt,
                };
                self.container_set(container, Value::Number(result), env, me)?;
            }
            Stmt::If {
                cond,
                then_body,
                else_body,
            } => {
                let c = self.eval(cond, env, me)?;
                let body = if c.as_bool() { then_body } else { else_body };
                return self.exec_stmts(body, env, me);
            }
            Stmt::Repeat { kind, body } => return self.exec_repeat(kind, body, env, me),
            Stmt::Send(name, args) => return self.exec_send(name, args, env, me),
        }
        Ok(Flow::Next)
    }

    fn exec_repeat(
        &mut self,
        kind: &RepeatKind,
        body: &[Stmt],
        env: &mut Env,
        me: Me,
    ) -> Result<Flow, String> {
        match kind {
            RepeatKind::Times(n) => {
                let count = self.eval(n, env, me)?.as_number().unwrap_or(0.0);
                let count = count.max(0.0) as i64;
                for _ in 0..count {
                    match self.exec_stmts(body, env, me)? {
                        Flow::ExitRepeat => break,
                        Flow::ExitHandler => return Ok(Flow::ExitHandler),
                        Flow::Next => {}
                    }
                }
            }
            RepeatKind::With { var, from, to } => {
                let start = self.eval(from, env, me)?.as_number().unwrap_or(0.0) as i64;
                let end = self.eval(to, env, me)?.as_number().unwrap_or(0.0) as i64;
                let mut i = start;
                while i <= end {
                    env.vars.insert(var.clone(), Value::Number(i as f64));
                    match self.exec_stmts(body, env, me)? {
                        Flow::ExitRepeat => break,
                        Flow::ExitHandler => return Ok(Flow::ExitHandler),
                        Flow::Next => {}
                    }
                    i += 1;
                }
            }
        }
        Ok(Flow::Next)
    }

    fn exec_send(
        &mut self,
        name: &str,
        args: &[Expr],
        _env: &mut Env,
        _me: Me,
    ) -> Result<Flow, String> {
        match name {
            "exit" => {
                if let Some(Expr::Var(w)) = args.first()
                    && w.eq_ignore_ascii_case("repeat")
                {
                    return Ok(Flow::ExitRepeat);
                }
                Ok(Flow::ExitHandler)
            }
            "return" | "pass" => Ok(Flow::ExitHandler),
            // Unknown custom commands are no-ops in the MVP runtime.
            _ => Ok(Flow::Next),
        }
    }

    fn exec_go(&mut self, dest: &Destination, env: &mut Env, me: Me) -> Result<(), String> {
        let n = self.stack.cards.len();
        if n == 0 {
            return Ok(());
        }
        let new = match dest {
            Destination::NextCard => (self.card_index + 1) % n,
            Destination::PrevCard => (self.card_index + n - 1) % n,
            Destination::FirstCard => 0,
            Destination::LastCard => n - 1,
            Destination::CardByNumber(e) => {
                let idx = self.eval(e, env, me)?.as_number().unwrap_or(1.0) as i64;
                ((idx - 1).rem_euclid(n as i64)) as usize
            }
            Destination::CardByName(e) => {
                let name = self.eval(e, env, me)?.as_text();
                self.stack
                    .cards
                    .iter()
                    .position(|c| c.name.eq_ignore_ascii_case(&name))
                    .ok_or_else(|| format!("no card named \"{name}\""))?
            }
        };
        self.card_index = new;
        Ok(())
    }

    // ---- containers ----

    fn container_get(&mut self, c: &Container, env: &mut Env, me: Me) -> Result<Value, String> {
        match c {
            Container::Variable(name) => Ok(env.vars.get(name).cloned().unwrap_or(Value::Empty)),
            Container::It => Ok(env.it.clone()),
            Container::Field(f) => Ok(Value::from_text(self.field_text(f, env, me)?)),
        }
    }

    fn container_set(
        &mut self,
        c: &Container,
        v: Value,
        env: &mut Env,
        me: Me,
    ) -> Result<(), String> {
        match c {
            Container::Variable(name) => {
                env.vars.insert(name.clone(), v);
            }
            Container::It => env.it = v,
            Container::Field(f) => self.set_field_text(f, v.as_text(), env, me)?,
        }
        Ok(())
    }

    // ---- field & button resolution ----

    fn eval_selector(
        &mut self,
        sel: &Selector,
        env: &mut Env,
        me: Me,
    ) -> Result<(bool, Value), String> {
        match sel {
            Selector::ByNumber(e) => Ok((true, self.eval(e, env, me)?)),
            Selector::ByName(e) => Ok((false, self.eval(e, env, me)?)),
        }
    }

    fn locate_field(&mut self, f: &FieldRef, env: &mut Env, me: Me) -> Result<Loc, String> {
        let (by_num, v) = self.eval_selector(&f.selector, env, me)?;
        match f.layer {
            Layer::Card => {
                let card = &self.stack.cards[self.card_index];
                let idx = find_index(card.fields.iter().map(|x| x.name.as_str()), by_num, &v)?;
                Ok(Loc::CardObj(idx))
            }
            Layer::Background => {
                let bg_id = self.stack.cards[self.card_index]
                    .background_id
                    .ok_or("card has no background")?;
                let bg_idx = self
                    .stack
                    .backgrounds
                    .iter()
                    .position(|b| b.id == bg_id)
                    .ok_or("background not found")?;
                let idx = find_index(
                    self.stack.backgrounds[bg_idx]
                        .fields
                        .iter()
                        .map(|x| x.name.as_str()),
                    by_num,
                    &v,
                )?;
                Ok(Loc::BgObj(bg_idx, idx))
            }
        }
    }

    fn field_text(&mut self, f: &FieldRef, env: &mut Env, me: Me) -> Result<String, String> {
        match self.locate_field(f, env, me)? {
            Loc::CardObj(i) => Ok(self.stack.cards[self.card_index].fields[i].text.clone()),
            Loc::BgObj(b, i) => Ok(self.stack.backgrounds[b].fields[i].text.clone()),
        }
    }

    fn set_field_text(
        &mut self,
        f: &FieldRef,
        text: String,
        env: &mut Env,
        me: Me,
    ) -> Result<(), String> {
        match self.locate_field(f, env, me)? {
            Loc::CardObj(i) => self.stack.cards[self.card_index].fields[i].text = text,
            Loc::BgObj(b, i) => self.stack.backgrounds[b].fields[i].text = text,
        }
        Ok(())
    }

    fn locate_button(&mut self, b: &ButtonRef, env: &mut Env, me: Me) -> Result<Loc, String> {
        let (by_num, v) = self.eval_selector(&b.selector, env, me)?;
        match b.layer {
            Layer::Card => {
                let card = &self.stack.cards[self.card_index];
                let idx = find_index(card.buttons.iter().map(|x| x.name.as_str()), by_num, &v)?;
                Ok(Loc::CardObj(idx))
            }
            Layer::Background => {
                let bg_id = self.stack.cards[self.card_index]
                    .background_id
                    .ok_or("card has no background")?;
                let bg_idx = self
                    .stack
                    .backgrounds
                    .iter()
                    .position(|bb| bb.id == bg_id)
                    .ok_or("background not found")?;
                let idx = find_index(
                    self.stack.backgrounds[bg_idx]
                        .buttons
                        .iter()
                        .map(|x| x.name.as_str()),
                    by_num,
                    &v,
                )?;
                Ok(Loc::BgObj(bg_idx, idx))
            }
        }
    }

    // ---- properties ----

    fn get_property(
        &mut self,
        name: &str,
        target: &ObjectRef,
        env: &mut Env,
        me: Me,
    ) -> Result<Value, String> {
        let prop = name.to_ascii_lowercase();
        let obj = self.resolve_object(target, me);
        match obj {
            ResolvedObj::Field(fref) => {
                let loc = self.locate_field(&fref, env, me)?;
                let (text, fname, vis) = self.field_props(&loc, env, me)?;
                Ok(match prop.as_str() {
                    "text" | "value" | "contents" => Value::from_text(text),
                    "name" | "short name" | "long name" => Value::from_text(fname),
                    "visible" | "vis022" => Value::Bool(vis),
                    _ => Value::Empty,
                })
            }
            ResolvedObj::Button(bref) => {
                let loc = self.locate_button(&bref, env, me)?;
                let (bname, title, vis) = match loc {
                    Loc::CardObj(i) => {
                        let b = &self.stack.cards[self.card_index].buttons[i];
                        (b.name.clone(), b.label().to_string(), b.visible)
                    }
                    Loc::BgObj(bi, i) => {
                        let b = &self.stack.backgrounds[bi].buttons[i];
                        (b.name.clone(), b.label().to_string(), b.visible)
                    }
                };
                Ok(match prop.as_str() {
                    "name" | "short name" | "long name" => Value::from_text(bname),
                    "title" | "text" | "label" => Value::from_text(title),
                    "visible" => Value::Bool(vis),
                    _ => Value::Empty,
                })
            }
            ResolvedObj::Card => {
                let card = &self.stack.cards[self.card_index];
                Ok(match prop.as_str() {
                    "name" | "short name" | "long name" => Value::from_text(card.name.clone()),
                    "number" => Value::Number((self.card_index + 1) as f64),
                    _ => Value::Empty,
                })
            }
            ResolvedObj::Stack => Ok(match prop.as_str() {
                "name" | "short name" | "long name" => Value::from_text(self.stack.name.clone()),
                "number" => Value::Number(self.stack.cards.len() as f64),
                _ => Value::Empty,
            }),
        }
    }

    fn set_property(
        &mut self,
        name: &str,
        target: &ObjectRef,
        v: Value,
        env: &mut Env,
        me: Me,
    ) -> Result<(), String> {
        let prop = name.to_ascii_lowercase();
        let obj = self.resolve_object(target, me);
        match obj {
            ResolvedObj::Field(fref) => {
                let loc = self.locate_field(&fref, env, me)?;
                let field = match loc {
                    Loc::CardObj(i) => &mut self.stack.cards[self.card_index].fields[i],
                    Loc::BgObj(b, i) => &mut self.stack.backgrounds[b].fields[i],
                };
                match prop.as_str() {
                    "text" | "value" | "contents" => field.text = v.as_text(),
                    "name" => field.name = v.as_text(),
                    "visible" => field.visible = v.as_bool(),
                    "locked" => field.locked = v.as_bool(),
                    _ => return Err(format!("unknown field property '{prop}'")),
                }
            }
            ResolvedObj::Button(bref) => {
                let loc = self.locate_button(&bref, env, me)?;
                let button = match loc {
                    Loc::CardObj(i) => &mut self.stack.cards[self.card_index].buttons[i],
                    Loc::BgObj(b, i) => &mut self.stack.backgrounds[b].buttons[i],
                };
                match prop.as_str() {
                    "title" | "text" | "label" => button.title = v.as_text(),
                    "name" => button.name = v.as_text(),
                    "visible" => button.visible = v.as_bool(),
                    _ => return Err(format!("unknown button property '{prop}'")),
                }
            }
            ResolvedObj::Card => {
                let card = &mut self.stack.cards[self.card_index];
                match prop.as_str() {
                    "name" => card.name = v.as_text(),
                    _ => return Err(format!("unknown card property '{prop}'")),
                }
            }
            ResolvedObj::Stack => match prop.as_str() {
                "name" => self.stack.name = v.as_text(),
                _ => return Err(format!("unknown stack property '{prop}'")),
            },
        }
        Ok(())
    }

    fn field_props(
        &mut self,
        loc: &Loc,
        _env: &mut Env,
        _me: Me,
    ) -> Result<(String, String, bool), String> {
        Ok(match loc {
            Loc::CardObj(i) => {
                let f = &self.stack.cards[self.card_index].fields[*i];
                (f.text.clone(), f.name.clone(), f.visible)
            }
            Loc::BgObj(b, i) => {
                let f = &self.stack.backgrounds[*b].fields[*i];
                (f.text.clone(), f.name.clone(), f.visible)
            }
        })
    }

    /// Resolve an `ObjectRef` (possibly `me`) into a concrete object descriptor we can
    /// locate. `me` resolves to a same-layer reference by id.
    fn resolve_object(&self, target: &ObjectRef, me: Me) -> ResolvedObj {
        match target {
            ObjectRef::Me => match me {
                Me::Button(id) => ResolvedObj::Button(ButtonRef {
                    layer: Layer::Card,
                    selector: Selector::ByName(Expr::Var(self.button_name(id))),
                }),
                Me::Field(id) => ResolvedObj::Field(FieldRef {
                    layer: Layer::Card,
                    selector: Selector::ByName(Expr::Str(self.field_name(id))),
                }),
                Me::Card | Me::Background(_) => ResolvedObj::Card,
                Me::Stack => ResolvedObj::Stack,
            },
            ObjectRef::Field(f) => ResolvedObj::Field(f.clone()),
            ObjectRef::Button(b) => ResolvedObj::Button(b.clone()),
            ObjectRef::Card => ResolvedObj::Card,
            ObjectRef::Stack => ResolvedObj::Stack,
        }
    }

    fn button_name(&self, id: u32) -> String {
        self.stack.cards[self.card_index]
            .buttons
            .iter()
            .find(|b| b.id == id)
            .map(|b| b.name.clone())
            .unwrap_or_default()
    }

    fn field_name(&self, id: u32) -> String {
        self.stack.cards[self.card_index]
            .fields
            .iter()
            .find(|f| f.id == id)
            .map(|f| f.name.clone())
            .unwrap_or_default()
    }

    // ---- expressions ----

    fn eval(&mut self, e: &Expr, env: &mut Env, me: Me) -> Result<Value, String> {
        match e {
            Expr::Number(n) => Ok(Value::Number(*n)),
            Expr::Str(s) => Ok(Value::from_text(s.clone())),
            Expr::Var(name) => Ok(self.eval_var(name, env)),
            Expr::Unary(op, inner) => {
                let v = self.eval(inner, env, me)?;
                Ok(match op {
                    UnOp::Neg => Value::Number(-v.as_number().ok_or("negate of non-number")?),
                    UnOp::Not => Value::Bool(!v.as_bool()),
                })
            }
            Expr::Binary(op, a, b) => {
                let lhs = self.eval(a, env, me)?;
                let rhs = self.eval(b, env, me)?;
                eval_binop(*op, lhs, rhs)
            }
            Expr::Property { name, target } => self.get_property(name, target, env, me),
            Expr::FieldContents(f) => Ok(Value::from_text(self.field_text(f.as_ref(), env, me)?)),
            Expr::Call(name, args) => self.eval_call(name, args, env, me),
        }
    }

    fn eval_var(&self, name: &str, env: &Env) -> Value {
        match name {
            "it" => env.it.clone(),
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            "empty" => Value::Empty,
            "space" => Value::from_text(" "),
            "tab" => Value::from_text("\t"),
            "return" | "cr" => Value::from_text("\n"),
            "quote" => Value::from_text("\""),
            "comma" => Value::from_text(","),
            "colon" => Value::from_text(":"),
            _ => env
                .vars
                .get(name)
                .cloned()
                // Undeclared bare words evaluate to their literal name, per HyperTalk.
                .unwrap_or_else(|| Value::from_text(name)),
        }
    }

    fn eval_call(
        &mut self,
        name: &str,
        args: &[Expr],
        env: &mut Env,
        me: Me,
    ) -> Result<Value, String> {
        match name {
            "number_of" => {
                let kind = args
                    .first()
                    .map(|e| match e {
                        Expr::Str(s) => s.to_ascii_lowercase(),
                        _ => String::new(),
                    })
                    .unwrap_or_default();
                let n = match kind.as_str() {
                    "cards" | "card" => self.stack.cards.len(),
                    "buttons" | "button" => self.stack.cards[self.card_index].buttons.len(),
                    "fields" | "field" => self.stack.cards[self.card_index].fields.len(),
                    "backgrounds" | "background" => self.stack.backgrounds.len(),
                    _ => 0,
                };
                Ok(Value::Number(n as f64))
            }
            "length" => {
                let s = self.eval(args.first().ok_or("length() needs an argument")?, env, me)?;
                Ok(Value::Number(s.as_text().chars().count() as f64))
            }
            "random" => {
                let max = self
                    .eval(args.first().ok_or("random() needs an argument")?, env, me)?
                    .as_number()
                    .unwrap_or(1.0)
                    .max(1.0) as u64;
                self.rng ^= self.rng << 13;
                self.rng ^= self.rng >> 7;
                self.rng ^= self.rng << 17;
                Ok(Value::Number((self.rng % max + 1) as f64))
            }
            "trunc" => {
                let n = self.eval(args.first().ok_or("trunc() needs an argument")?, env, me)?;
                Ok(Value::Number(n.as_number().unwrap_or(0.0).trunc()))
            }
            other => Err(format!("unknown function '{other}'")),
        }
    }
}

/// A resolved object target, ready to be located.
enum ResolvedObj {
    Field(FieldRef),
    Button(ButtonRef),
    Card,
    Stack,
}

/// Find an object's index within a layer by 1-based number or by name (case-insensitive).
fn find_index<'a>(
    names: impl Iterator<Item = &'a str>,
    by_number: bool,
    v: &Value,
) -> Result<usize, String> {
    if by_number {
        let n = v.as_number().ok_or("non-numeric object index")? as i64;
        if n < 1 {
            return Err(format!("object index out of range: {n}"));
        }
        Ok((n - 1) as usize)
    } else {
        let want = v.as_text();
        names
            .enumerate()
            .find(|(_, name)| name.eq_ignore_ascii_case(&want))
            .map(|(i, _)| i)
            .ok_or_else(|| format!("no object named \"{want}\""))
    }
}

fn eval_binop(op: BinOp, a: Value, b: Value) -> Result<Value, String> {
    use BinOp::*;
    let num = |x: &Value| x.as_number().ok_or_else(|| "expected a number".to_string());
    Ok(match op {
        Add => Value::Number(num(&a)? + num(&b)?),
        Sub => Value::Number(num(&a)? - num(&b)?),
        Mul => Value::Number(num(&a)? * num(&b)?),
        Div => Value::Number(num(&a)? / num(&b)?),
        Mod => Value::Number(num(&a)? % num(&b)?),
        Concat => Value::from_text(format!("{}{}", a.as_text(), b.as_text())),
        ConcatSpace => Value::from_text(format!("{} {}", a.as_text(), b.as_text())),
        And => Value::Bool(a.as_bool() && b.as_bool()),
        Or => Value::Bool(a.as_bool() || b.as_bool()),
        Eq => Value::Bool(values_equal(&a, &b)),
        Ne => Value::Bool(!values_equal(&a, &b)),
        Lt => Value::Bool(compare(&a, &b)? < 0),
        Gt => Value::Bool(compare(&a, &b)? > 0),
        Le => Value::Bool(compare(&a, &b)? <= 0),
        Ge => Value::Bool(compare(&a, &b)? >= 0),
    })
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a.as_number(), b.as_number()) {
        (Some(x), Some(y)) => x == y,
        _ => a.as_text().eq_ignore_ascii_case(&b.as_text()),
    }
}

/// Returns -1, 0, or 1. Numeric when both coerce to numbers, else case-insensitive text.
fn compare(a: &Value, b: &Value) -> Result<i32, String> {
    match (a.as_number(), b.as_number()) {
        (Some(x), Some(y)) => Ok(x.partial_cmp(&y).map(|o| o as i32).unwrap_or(0)),
        _ => Ok(a.as_text().to_lowercase().cmp(&b.as_text().to_lowercase()) as i32),
    }
}
