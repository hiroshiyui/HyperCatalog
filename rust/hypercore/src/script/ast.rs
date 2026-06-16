//! Abstract syntax for the HyperTalk subset.

#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    pub handlers: Vec<Handler>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Handler {
    /// Message name, lowercased (e.g. "mouseup", "opencard").
    pub message: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Layer {
    Card,
    Background,
}

/// How a field or button is selected within its layer.
#[derive(Clone, Debug, PartialEq)]
pub enum Selector {
    ByName(Expr),
    ByNumber(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldRef {
    pub layer: Layer,
    pub selector: Selector,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ButtonRef {
    pub layer: Layer,
    pub selector: Selector,
}

/// A place a value can be written to.
#[derive(Clone, Debug, PartialEq)]
pub enum Container {
    Variable(String),
    It,
    Field(FieldRef),
}

/// Any object a property can be read from / written to.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectRef {
    Me,
    Field(FieldRef),
    Button(ButtonRef),
    Card,
    Stack,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Destination {
    NextCard,
    PrevCard,
    FirstCard,
    LastCard,
    CardByName(Expr),
    CardByNumber(Expr),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArithOp {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Concat,      // &
    ConcatSpace, // &&
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    /// `put EXPR [into|before|after CONTAINER]`. None container = message box.
    Put {
        value: Expr,
        container: Option<Container>,
    },
    /// `get EXPR` -> stores into `it`.
    Get(Expr),
    /// `set PROP of OBJECT to EXPR`.
    Set {
        prop: String,
        target: ObjectRef,
        value: Expr,
    },
    /// `go [to] DESTINATION`.
    Go(Destination),
    /// `answer EXPR`.
    Answer(Expr),
    /// `beep`.
    Beep,
    /// `add EXPR to CONTAINER`, etc.
    Arith {
        op: ArithOp,
        amount: Expr,
        container: Container,
    },
    If {
        cond: Expr,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
    },
    /// `repeat [with v = a to b] | [N times]` ... `end repeat`.
    Repeat { kind: RepeatKind, body: Vec<Stmt> },
    /// A bare message send (e.g. a custom command or `mouseUp`).
    Send(String, Vec<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum RepeatKind {
    Times(Expr),
    With { var: String, from: Expr, to: Expr },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Str(String),
    Number(f64),
    /// A bare word: variable, `it`, or a literal constant (true/false/empty/...).
    Var(String),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `the PROP of OBJECT`.
    Property {
        name: String,
        target: Box<ObjectRef>,
    },
    /// Reading a field's contents as a value. Boxed to break the
    /// `Expr → FieldRef → Selector → Expr` type cycle.
    FieldContents(Box<FieldRef>),
    /// `the number of cards`, `length(x)`, etc.
    Call(String, Vec<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
}
