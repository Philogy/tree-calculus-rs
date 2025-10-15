use crate::lexer::{Lexer, Token};

pub enum TreeExpr<'src> {
    Leaf,
    Application(Vec<TreeExpr<'src>>),
    NameRef(&'src str),
    Lambda(TreeLambda<'src>),
}

pub struct TreeLambda<'src> {
    params: Vec<&'src str>,
    body: Box<TreeExpr<'src>>,
}

pub type Error = (String, std::ops::Range<usize>);

fn parse_tree_expr<'src>(
    lexer: &mut Lexer<'src>,
    inside_paren: bool,
) -> Result<TreeExpr<'src>, Error> {
    let mut trees = Vec::with_capacity(8);
    loop {
        let t = match lexer.peek() {
            (Token::Identifier(name), _) => {
                lexer.next();
                TreeExpr::NameRef(name)
            }
            (Token::Triangle, _) => {
                lexer.next();
                TreeExpr::Leaf
            }
            (Token::Fn, _) => {
                lexer.next();
                let mut params = Vec::with_capacity(4);
                loop {
                    match lexer.next_skip_newline() {
                        (Token::Identifier(name), _) => params.push(name),
                        (Token::Arrow, _) => break,
                        (tok, span) => {
                            return Err((
                                format!("Unexpected token {:?}, expected <name> / '->'", tok),
                                span,
                            ));
                        }
                    }
                }
                let body = parse_tree_expr(lexer, inside_paren)?;
                TreeExpr::Lambda(TreeLambda {
                    params,
                    body: Box::new(body),
                })
            }
            (Token::Open, _) => {
                let inner_expr = parse_tree_expr(lexer, true)?;
                let (tok, span) = lexer.next();
                if !matches!(tok, Token::Close) {
                    return Err((
                        format!("Missing closing ')', got unexpected: <{tok:?}>"),
                        span,
                    ));
                }
                inner_expr
            }
            (Token::Close, _) if inside_paren => break,
            (Token::Newline, _) if !inside_paren && !trees.is_empty() => break,
            (Token::Newline, _) if inside_paren => continue,
            (Token::Eof, _) => break,
            (tok, span) => {
                return Err((
                    format!("Unexpected token {:?}, expected △ / (tree) / name", tok),
                    span,
                ));
            }
        };
        trees.push(t);
    }
    Ok(TreeExpr::Application(trees))
}

pub struct TreeDecl<'src> {
    name: &'src str,
    expr: TreeExpr<'src>,
}

fn parse_decl<'src>(lexer: &mut Lexer<'src>) -> Result<TreeDecl<'src>, Error> {
    let (tok, span) = lexer.next_skip_newline();
    let Token::Identifier(name) = tok else {
        return Err((format!("Unexpected token <{tok:?}>, expected <name>"), span));
    };
    let (tok, span) = lexer.next_skip_newline();
    let Token::Equal = tok else {
        return Err((format!("Unexpected token <{tok:?}>, expected '='"), span));
    };
    let expr = parse_tree_expr(lexer, false)?;

    Ok(TreeDecl { name, expr })
}
