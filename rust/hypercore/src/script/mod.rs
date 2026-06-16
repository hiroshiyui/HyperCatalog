//! HyperTalk scripting: lexer → parser → AST → interpreter.

pub mod ast;
pub mod interp;
pub mod lexer;
pub mod parser;
pub mod value;

pub use interp::{HostCmd, Me, Runtime};
pub use parser::parse_script;
pub use value::Value;
