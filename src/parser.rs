use crate::lexer::{Lexer, Span, Token};
use std::fmt;

#[derive(Debug, Clone)]
pub enum TreeExpr<'src> {
    Leaf,
    Application(Vec<TreeExpr<'src>>),
    NameRef((&'src str, Span)),
    Lambda(TreeLambda<'src>),
}

impl<'src> fmt::Display for TreeExpr<'src> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Leaf => write!(f, "△")?,
            Self::NameRef((name, _)) => write!(f, "{}", name)?,
            Self::Application(sub_exprs) => {
                if sub_exprs.len() == 1 {
                    write!(f, "{}", sub_exprs[0])?;
                } else {
                    for (i, sub_expr) in sub_exprs.into_iter().enumerate() {
                        if i > 0 {
                            write!(f, " ")?;
                        }
                        if matches!(sub_expr, Self::Application(_) | Self::Lambda(_)) {
                            write!(f, "({})", sub_expr)?;
                        } else {
                            write!(f, "{}", sub_expr)?;
                        }
                    }
                }
            }
            Self::Lambda(lambda) => write!(f, "{}", lambda)?,
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TreeLambda<'src> {
    pub params: Vec<&'src str>,
    pub body: Box<TreeExpr<'src>>,
}

pub type Error = (String, Span);

impl<'src> fmt::Display for TreeLambda<'src> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fn ")?;
        for (i, param) in self.params.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{}", param)?;
        }
        write!(f, " -> {}", self.body)?;

        Ok(())
    }
}

const MAX_APPLICATION_CHAIN: usize = 10_000;

fn parse_tree_expr<'src>(
    lexer: &mut Lexer<'src>,
    inside_paren: bool,
) -> Result<TreeExpr<'src>, Error> {
    let mut trees = Vec::with_capacity(8);
    let mut applications = 0;
    loop {
        applications += 1;
        assert!(applications < MAX_APPLICATION_CHAIN, "too many iters");

        let t = match lexer.peek() {
            (Token::Identifier(name), span) => {
                lexer.next();
                TreeExpr::NameRef((name, span))
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
                lexer.next();
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
            (Token::Newline, _) if inside_paren => {
                lexer.next();
                continue;
            }
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

#[derive(Debug)]
pub struct TreeDecl<'src> {
    pub name: &'src str,
    pub span: Span,
    pub expr: TreeExpr<'src>,
}

pub fn parse_decl<'src>(lexer: &mut Lexer<'src>) -> Result<TreeDecl<'src>, Error> {
    let (tok, name_span) = lexer.next_skip_newline();
    let Token::Identifier(name) = tok else {
        return Err((
            format!("Unexpected token <{tok:?}>, expected <name>"),
            name_span,
        ));
    };
    let (tok, span) = lexer.next_skip_newline();
    let Token::Equal = tok else {
        return Err((format!("Unexpected token <{tok:?}>, expected '='"), span));
    };
    let expr = parse_tree_expr(lexer, false)?;

    Ok(TreeDecl {
        name,
        span: name_span,
        expr,
    })
}

pub fn parse_declarations<'src>(lexer: &mut Lexer<'src>) -> Result<Vec<TreeDecl<'src>>, Error> {
    let mut decls = Vec::with_capacity(128);

    while let (Token::Identifier(_), _) = lexer.peek_skip_newline() {
        decls.push(parse_decl(lexer)?);
    }

    let (tok, span) = lexer.next_skip_newline();
    if !matches!(tok, Token::Eof) {
        return Err((
            format!("Unexpected token: <{tok:?}>, expected EOF / declaration"),
            span,
        ));
    }

    Ok(decls)
}
