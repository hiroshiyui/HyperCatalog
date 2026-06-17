//! The HyperTalk interpreter / runtime. A `Runtime` borrows the document `Stack`,
//! tracks the current card, executes handler bodies, and accumulates host effects
//! (dialogs, beeps, message-box output) for the platform host to perform.

use std::collections::HashMap;

use super::ast::*;
use super::parser::parse_script;
use super::value::Value;
use crate::model::{Rect, Stack};

/// Minimum width/height a script may set via a geometry property, so `set the rect/width`
/// can't produce a zero or negative size that breaks hit-testing or rendering.
const MIN_GEOM_SIZE: f32 = 1.0;

/// Side effects the core cannot perform itself; the host carries them out.
#[derive(Clone, Debug, PartialEq)]
pub enum HostCmd {
    /// `answer "..."` — show a modal dialog.
    Answer(String),
    /// `beep`.
    Beep,
    /// `put "..."` with no container — message-box output.
    Message(String),
    /// `go [to] stack "Name"` — the host loads the named stack (the core has no asset access).
    GoStack(String),
    /// `show stacks` — the host opens its stack picker.
    ShowStacks,
    /// `open url "…"` — the host opens the URL in a browser/viewer (ADR-0023).
    OpenUrl(String),
    /// `share "…"` — the host opens the system share sheet (ADR-0023).
    Share(String),
    /// `toast "…"` — the host shows a brief toast (ADR-0023).
    Toast(String),
    /// `get url "…"` — the host fetches the URL off-thread and delivers the body back via
    /// `on responseReceived data` (ADR-0025).
    GetUrl(String),
    /// `ask permission "name"` — the host requests the runtime permission and delivers the outcome
    /// via `on permissionResult name, granted` (ADR-0025).
    AskPermission(String),
    /// `snackbar text [action label send msg]` — a Material snackbar; tapping its action (if any)
    /// fires the named message (ADR-0025). Fields: text, action label, action message.
    Snackbar(String, String, String),
    /// `notify title, body [send msg]` — post a notification; tapping it (if `msg` non-empty) fires
    /// the named message (ADR-0025). Fields: title, body, tap message.
    Notify(String, String, String),
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

/// Upper bound on total loop iterations per handler run. A runaway/typo loop (e.g.
/// `repeat with i = 1 to 1000000000`) would otherwise block the host UI thread, since
/// scripts run synchronously on a tap. Covers all (incl. nested) repeats in one dispatch.
const MAX_LOOP_ITERATIONS: u64 = 1_000_000;

/// Maximum nesting of custom `send` re-dispatch in one run. Guards against a handler that sends
/// its own message (`on a / send "a"`), which would otherwise recurse until the stack overflows.
const MAX_SEND_DEPTH: u32 = 64;

pub struct Runtime<'s> {
    pub stack: &'s mut Stack,
    pub card_index: usize,
    pub host: Vec<HostCmd>,
    /// Simple deterministic PRNG state for `random()` (we avoid OS randomness).
    rng: u64,
    /// Remaining loop iterations for this run (reset each `Runtime::new`).
    loop_budget: u64,
    /// The message path (cloned script sources + their `me`), so a running handler can re-dispatch
    /// a custom `send` along the same object → card → background → stack chain (ADR-0024).
    path: Vec<(String, Me)>,
    /// Current custom-`send` recursion depth (bounded by `MAX_SEND_DEPTH`).
    send_depth: u32,
}

impl<'s> Runtime<'s> {
    pub fn new(stack: &'s mut Stack, card_index: usize, path: Vec<(String, Me)>) -> Runtime<'s> {
        Runtime {
            stack,
            card_index,
            host: Vec::new(),
            rng: 0x2545_F491_4F6C_DD1D,
            loop_budget: MAX_LOOP_ITERATIONS,
            path,
            send_depth: 0,
        }
    }

    /// Consume one unit of the loop budget; errors if a run exhausts it.
    fn tick_loop(&mut self) -> Result<(), String> {
        if self.loop_budget == 0 {
            return Err(format!(
                "repeat exceeded the maximum of {MAX_LOOP_ITERATIONS} iterations"
            ));
        }
        self.loop_budget -= 1;
        Ok(())
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
                    self.tick_loop()?;
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
                    self.tick_loop()?;
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
        env: &mut Env,
        me: Me,
    ) -> Result<Flow, String> {
        // Single-string-arg host effects: escape hatches (ADR-0023) + async requests (ADR-0025).
        let host_effect = match name {
            "openurl" => Some(HostCmd::OpenUrl as fn(String) -> HostCmd),
            "share" => Some(HostCmd::Share as fn(String) -> HostCmd),
            "toast" => Some(HostCmd::Toast as fn(String) -> HostCmd),
            "geturl" => Some(HostCmd::GetUrl as fn(String) -> HostCmd),
            "askpermission" => Some(HostCmd::AskPermission as fn(String) -> HostCmd),
            _ => None,
        };
        if let Some(make) = host_effect {
            let text = match args.first() {
                Some(e) => self.eval(e, env, me)?.as_text(),
                None => String::new(),
            };
            self.host.push(make(text));
            return Ok(Flow::Next);
        }
        // Multi-arg async requests (ADR-0025): evaluate the first three positional args (missing →
        // empty), then push the request effect for the host to perform.
        if name == "snackbar" || name == "notify" {
            let mut a = ["".to_string(), "".to_string(), "".to_string()];
            for (i, slot) in a.iter_mut().enumerate() {
                if let Some(e) = args.get(i) {
                    *slot = self.eval(e, env, me)?.as_text();
                }
            }
            let [x, y, z] = a;
            self.host.push(match name {
                "snackbar" => HostCmd::Snackbar(x, y, z),
                _ => HostCmd::Notify(x, y, z),
            });
            return Ok(Flow::Next);
        }
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
            "show" => {
                // `show stacks` asks the host to open its stack picker; the core has no UI.
                if let Some(Expr::Var(w)) = args.first()
                    && w.eq_ignore_ascii_case("stacks")
                {
                    self.host.push(HostCmd::ShowStacks);
                }
                Ok(Flow::Next)
            }
            // Any other name is a **custom message**: evaluate its arguments and re-dispatch it
            // along the current message path (ADR-0024). An unmatched name is a silent no-op,
            // preserving the old behavior for typos / not-yet-defined handlers.
            _ => {
                let vals = args
                    .iter()
                    .map(|e| self.eval(e, env, me))
                    .collect::<Result<Vec<_>, _>>()?;
                self.send_custom(name, &vals)?;
                Ok(Flow::Next)
            }
        }
    }

    /// Re-dispatch a custom message along the message path the runtime was built with: the first
    /// object whose script defines `on <name>` runs it, with `me` bound to **that** object (its
    /// defining owner, per HyperCard semantics — not the sender). Bounded by `MAX_SEND_DEPTH`.
    fn send_custom(&mut self, name: &str, args: &[Value]) -> Result<bool, String> {
        if self.send_depth >= MAX_SEND_DEPTH {
            return Err(format!(
                "send exceeded the maximum nesting of {MAX_SEND_DEPTH} (a handler may send itself)"
            ));
        }
        self.send_depth += 1;
        // Clone the path so we can call `&mut self` handlers while iterating it.
        let path = self.path.clone();
        let mut ran = false;
        let mut err = None;
        for (src, owner) in &path {
            match self.run_handler(src, name, *owner, args) {
                Ok(true) => {
                    ran = true;
                    break;
                }
                Ok(false) => continue,
                Err(e) => {
                    err = Some(e);
                    break;
                }
            }
        }
        self.send_depth -= 1;
        match err {
            Some(e) => Err(e),
            None => Ok(ran),
        }
    }

    fn exec_go(&mut self, dest: &Destination, env: &mut Env, me: Me) -> Result<(), String> {
        // Switching stacks is a host effect: the core has no asset access to load another
        // stack, so it asks the host (which resolves the name and reloads the session).
        if let Destination::Stack(e) = dest {
            let name = self.eval(e, env, me)?.as_text();
            self.host.push(HostCmd::GoStack(name));
            return Ok(());
        }
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
            // Handled above; the card-index logic doesn't apply to a stack switch.
            Destination::Stack(_) => return Ok(()),
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
                let f = match loc {
                    Loc::CardObj(i) => &self.stack.cards[self.card_index].fields[i],
                    Loc::BgObj(b, i) => &self.stack.backgrounds[b].fields[i],
                };
                Ok(match prop.as_str() {
                    "text" | "value" | "contents" => Value::from_text(f.text.clone()),
                    "name" | "short name" | "long name" => Value::from_text(f.name.clone()),
                    "visible" => Value::Bool(f.visible),
                    "locked" => Value::Bool(f.locked),
                    "id" => Value::Number(f.id as f64),
                    "textfont" => Value::from_text(f.text_font.clone()),
                    "textsize" => Value::Number(f.text_size as f64),
                    "textstyle" => Value::from_text(text_style_get(&f.text_style)),
                    "textalign" => Value::from_text(f.text_align.clone()),
                    "weight" => Value::Number(f.weight as f64),
                    "textrole" => Value::from_text(f.text_role.clone()),
                    "contentdescription" => Value::from_text(f.content_description.clone()),
                    "liveregion" => Value::from_text(f.live_region.clone()),
                    _ => geom_get(&prop, f.rect).unwrap_or(Value::Empty),
                })
            }
            ResolvedObj::Button(bref) => {
                let loc = self.locate_button(&bref, env, me)?;
                let b = match loc {
                    Loc::CardObj(i) => &self.stack.cards[self.card_index].buttons[i],
                    Loc::BgObj(bi, i) => &self.stack.backgrounds[bi].buttons[i],
                };
                Ok(match prop.as_str() {
                    "name" | "short name" | "long name" => Value::from_text(b.name.clone()),
                    "title" | "text" | "label" => Value::from_text(b.label().to_string()),
                    "checked" => Value::Bool(b.checked.unwrap_or(false)),
                    "value" => Value::Number(b.value.unwrap_or(0.0) as f64),
                    "control" => Value::from_text(b.control.clone()),
                    "source" => Value::from_text(b.source.clone()),
                    "contentdescription" => Value::from_text(b.content_description.clone()),
                    "role" => Value::from_text(b.role.clone()),
                    "visible" => Value::Bool(b.visible),
                    "id" => Value::Number(b.id as f64),
                    "textfont" => Value::from_text(b.text_font.clone()),
                    "textsize" => Value::Number(b.text_size as f64),
                    "textstyle" => Value::from_text(text_style_get(&b.text_style)),
                    "textalign" => Value::from_text(b.text_align.clone()),
                    "weight" => Value::Number(b.weight as f64),
                    _ => geom_get(&prop, b.rect).unwrap_or(Value::Empty),
                })
            }
            ResolvedObj::Card => {
                let card = &self.stack.cards[self.card_index];
                Ok(match prop.as_str() {
                    "name" | "short name" | "long name" => Value::from_text(card.name.clone()),
                    "number" => Value::Number((self.card_index + 1) as f64),
                    // Card-level layout overlay (ADR-0016): the root container's mode/padding.
                    "layout" => Value::from_text(
                        card.layout
                            .as_ref()
                            .map(|l| l.mode.clone())
                            .unwrap_or_default(),
                    ),
                    "padding" => {
                        Value::Number(card.layout.as_ref().map(|l| l.padding).unwrap_or(0.0) as f64)
                    }
                    "safetop" => Value::Number(self.stack.safe_insets.top as f64),
                    "saferight" => Value::Number(self.stack.safe_insets.right as f64),
                    "safebottom" => Value::Number(self.stack.safe_insets.bottom as f64),
                    "safeleft" => Value::Number(self.stack.safe_insets.left as f64),
                    _ => Value::Empty,
                })
            }
            ResolvedObj::Stack => Ok(match prop.as_str() {
                "name" | "short name" | "long name" => Value::from_text(self.stack.name.clone()),
                "number" => Value::Number(self.stack.cards.len() as f64),
                "theme" => Value::from_text(self.stack.theme.clone()),
                "accentcolor" => Value::from_text(self.stack.accent_color.clone()),
                "safetop" => Value::Number(self.stack.safe_insets.top as f64),
                "saferight" => Value::Number(self.stack.safe_insets.right as f64),
                "safebottom" => Value::Number(self.stack.safe_insets.bottom as f64),
                "safeleft" => Value::Number(self.stack.safe_insets.left as f64),
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
                    "textfont" => field.text_font = v.as_text(),
                    "textsize" => {
                        if let Some(n) = v.as_number() {
                            field.text_size = n as f32;
                        }
                    }
                    "textstyle" => field.text_style = v.as_text(),
                    "textalign" => field.text_align = v.as_text(),
                    "weight" => {
                        if let Some(n) = v.as_number() {
                            field.weight = n as f32;
                        }
                    }
                    "textrole" => field.text_role = v.as_text(),
                    "contentdescription" => field.content_description = v.as_text(),
                    "liveregion" => field.live_region = v.as_text(),
                    _ => {
                        if !geom_set(&prop, &mut field.rect, &v) {
                            return Err(format!("unknown field property '{prop}'"));
                        }
                    }
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
                    "checked" => button.checked = Some(v.as_bool()),
                    "value" => {
                        if let Some(n) = v.as_number() {
                            button.value = Some(n as f32);
                        }
                    }
                    "control" => button.control = v.as_text(),
                    "source" => button.source = v.as_text(),
                    "contentdescription" => button.content_description = v.as_text(),
                    "role" => button.role = v.as_text(),
                    "visible" => button.visible = v.as_bool(),
                    "textfont" => button.text_font = v.as_text(),
                    "textsize" => {
                        if let Some(n) = v.as_number() {
                            button.text_size = n as f32;
                        }
                    }
                    "textstyle" => button.text_style = v.as_text(),
                    "textalign" => button.text_align = v.as_text(),
                    "weight" => {
                        if let Some(n) = v.as_number() {
                            button.weight = n as f32;
                        }
                    }
                    _ => {
                        if !geom_set(&prop, &mut button.rect, &v) {
                            return Err(format!("unknown button property '{prop}'"));
                        }
                    }
                }
            }
            ResolvedObj::Card => match prop.as_str() {
                "name" => self.stack.cards[self.card_index].name = v.as_text(),
                // `set the layout of this card to "column"|"row"|"grid"` (ADR-0016): build/replace a
                // single-level root layout over all the card's objects (nested authoring is YAML-only).
                "layout" => self.set_card_layout(&v.as_text().to_ascii_lowercase()),
                "padding" => {
                    if self.stack.cards[self.card_index].layout.is_none() {
                        self.set_card_layout("column");
                    }
                    if let Some(l) = &mut self.stack.cards[self.card_index].layout {
                        l.padding = v.as_number().unwrap_or(0.0) as f32;
                    }
                }
                _ => return Err(format!("unknown card property '{prop}'")),
            },
            ResolvedObj::Stack => match prop.as_str() {
                "name" => self.stack.name = v.as_text(),
                "theme" => self.stack.theme = v.as_text(),
                "accentcolor" => self.stack.accent_color = v.as_text(),
                _ => return Err(format!("unknown stack property '{prop}'")),
            },
        }
        Ok(())
    }

    /// Replace the current card's layout overlay with a single-level root group of `mode` over all
    /// its objects, in render order (background then card). Backs `set the layout of this card`
    /// (ADR-0016); preserves any existing root padding. A `grid` with no columns defaults to 2.
    fn set_card_layout(&mut self, mode: &str) {
        let idx = self.card_index;
        let mut ids: Vec<u32> = Vec::new();
        if let Some(bg) = self.stack.cards[idx]
            .background_id
            .and_then(|bid| self.stack.backgrounds.iter().find(|b| b.id == bid))
        {
            ids.extend(bg.fields.iter().map(|f| f.id));
            ids.extend(bg.buttons.iter().map(|b| b.id));
        }
        let card = &self.stack.cards[idx];
        ids.extend(card.fields.iter().map(|f| f.id));
        ids.extend(card.buttons.iter().map(|b| b.id));
        let padding = card.layout.as_ref().map(|l| l.padding).unwrap_or(0.0);
        let columns = if mode == "grid" { 2 } else { 0 };
        self.stack.cards[idx].layout = Some(crate::model::LayoutGroup {
            mode: mode.to_string(),
            padding,
            weight: 0.0,
            columns,
            children: ids
                .into_iter()
                .map(crate::model::LayoutChild::Object)
                .collect(),
        });
    }

    /// Resolve an `ObjectRef` (possibly `me`) into a concrete object descriptor we can
    /// locate. `me` resolves by 1-based position within whichever layer it lives on, so it
    /// works for both card and background objects and is immune to name collisions.
    fn resolve_object(&self, target: &ObjectRef, me: Me) -> ResolvedObj {
        match target {
            ObjectRef::Me => match me {
                Me::Button(id) => ResolvedObj::Button(self.me_button_ref(id)),
                Me::Field(id) => ResolvedObj::Field(self.me_field_ref(id)),
                Me::Card | Me::Background(_) => ResolvedObj::Card,
                Me::Stack => ResolvedObj::Stack,
            },
            ObjectRef::Field(f) => ResolvedObj::Field(f.clone()),
            ObjectRef::Button(b) => ResolvedObj::Button(b.clone()),
            ObjectRef::Card => ResolvedObj::Card,
            ObjectRef::Stack => ResolvedObj::Stack,
        }
    }

    fn me_button_ref(&self, id: u32) -> ButtonRef {
        let card = &self.stack.cards[self.card_index];
        if let Some(pos) = card.buttons.iter().position(|b| b.id == id) {
            return button_ref_by_number(Layer::Card, pos);
        }
        if let Some(bg) = card.background_id.and_then(|i| self.stack.background(i))
            && let Some(pos) = bg.buttons.iter().position(|b| b.id == id)
        {
            return button_ref_by_number(Layer::Background, pos);
        }
        // `me` should always be a live object; if not, an out-of-range ref errors cleanly.
        button_ref_by_number(Layer::Card, usize::MAX)
    }

    fn me_field_ref(&self, id: u32) -> FieldRef {
        let card = &self.stack.cards[self.card_index];
        if let Some(pos) = card.fields.iter().position(|f| f.id == id) {
            return field_ref_by_number(Layer::Card, pos);
        }
        if let Some(bg) = card.background_id.and_then(|i| self.stack.background(i))
            && let Some(pos) = bg.fields.iter().position(|f| f.id == id)
        {
            return field_ref_by_number(Layer::Background, pos);
        }
        field_ref_by_number(Layer::Card, usize::MAX)
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

/// A button reference selecting the (0-based) `pos`-th button of `layer`, by HyperTalk's
/// 1-based number. `pos as f64 + 1.0` (not `pos + 1`) avoids overflow on the sentinel.
fn button_ref_by_number(layer: Layer, pos: usize) -> ButtonRef {
    ButtonRef {
        layer,
        selector: Selector::ByNumber(Expr::Number(pos as f64 + 1.0)),
    }
}

fn field_ref_by_number(layer: Layer, pos: usize) -> FieldRef {
    FieldRef {
        layer,
        selector: Selector::ByNumber(Expr::Number(pos as f64 + 1.0)),
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
        let idx = (n - 1) as usize;
        // Upper-bound check: without it an out-of-range index would panic when the caller
        // indexes the slice, and a panic across the `extern "system"` FFI is UB.
        if idx >= names.count() {
            return Err(format!("object number out of range: {n}"));
        }
        Ok(idx)
    } else {
        let want = v.as_text();
        names
            .enumerate()
            .find(|(_, name)| name.eq_ignore_ascii_case(&want))
            .map(|(i, _)| i)
            .ok_or_else(|| format!("no object named \"{want}\""))
    }
}

/// `the textStyle` reads back as `"plain"` when no styles are set (HyperTalk convention).
fn text_style_get(style: &str) -> String {
    if style.is_empty() {
        "plain".to_string()
    } else {
        style.to_string()
    }
}

/// Read a geometric property off a rect. HyperTalk `loc`/`location` is the center point
/// `"h,v"`; `rect`/`rectangle` is `"left,top,right,bottom"`. Returns None for non-geometry.
fn geom_get(prop: &str, r: Rect) -> Option<Value> {
    let num = |x: f32| Value::Number(x as f64);
    Some(match prop {
        "loc" | "location" => Value::from_text(format!(
            "{},{}",
            num(r.x + r.w / 2.0).as_text(),
            num(r.y + r.h / 2.0).as_text(),
        )),
        "rect" | "rectangle" => Value::from_text(format!(
            "{},{},{},{}",
            num(r.x).as_text(),
            num(r.y).as_text(),
            num(r.x + r.w).as_text(),
            num(r.y + r.h).as_text(),
        )),
        "width" => num(r.w),
        "height" => num(r.h),
        "top" => num(r.y),
        "left" => num(r.x),
        "bottom" => num(r.y + r.h),
        "right" => num(r.x + r.w),
        _ => return None,
    })
}

/// Apply a geometric property to a rect. Moves keep size; `width`/`height` keep the
/// top-left corner; `loc` re-centers; `rect` sets all four edges. Returns false for a
/// non-geometry property (so the caller can report it as unknown). Malformed coordinate
/// strings are ignored (no change), matching HyperTalk's lenient `set`.
fn geom_set(prop: &str, r: &mut Rect, v: &Value) -> bool {
    match prop {
        "loc" | "location" => {
            if let Some([cx, cy]) = parse_coords::<2>(v) {
                r.x = cx - r.w / 2.0;
                r.y = cy - r.h / 2.0;
            }
        }
        "rect" | "rectangle" => {
            if let Some([l, t, right, bottom]) = parse_coords::<4>(v) {
                r.x = l;
                r.y = t;
                r.w = (right - l).max(MIN_GEOM_SIZE);
                r.h = (bottom - t).max(MIN_GEOM_SIZE);
            }
        }
        "width" => {
            if let Some(n) = v.as_number() {
                r.w = (n as f32).max(MIN_GEOM_SIZE);
            }
        }
        "height" => {
            if let Some(n) = v.as_number() {
                r.h = (n as f32).max(MIN_GEOM_SIZE);
            }
        }
        "top" => {
            if let Some(n) = v.as_number() {
                r.y = n as f32;
            }
        }
        "left" => {
            if let Some(n) = v.as_number() {
                r.x = n as f32;
            }
        }
        "bottom" => {
            if let Some(n) = v.as_number() {
                r.y = n as f32 - r.h;
            }
        }
        "right" => {
            if let Some(n) = v.as_number() {
                r.x = n as f32 - r.w;
            }
        }
        _ => return false,
    }
    true
}

/// Parse exactly `N` comma-separated numbers from a value's text (e.g. `"10,20,90,140"`).
fn parse_coords<const N: usize>(v: &Value) -> Option<[f32; N]> {
    let parts: Vec<f32> = v
        .as_text()
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    parts.try_into().ok()
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
