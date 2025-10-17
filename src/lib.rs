mod compiler;
mod fast_tree;
mod lexer;
pub mod old_tree;
pub mod optimizations;
mod parser;

pub use compiler::{CompiledTree, TreeNamespace, compile_tree_expr};
pub use fast_tree::*;
pub use lexer::{Lexer, Span};
pub use parser::{
    Declaration, Error as ParserError, TreeDecl, TreeExpr, TreeLambda, parse_decl,
    parse_declarations,
};
