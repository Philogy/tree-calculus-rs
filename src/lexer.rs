use std::ops::Range;

use logos::{Lexer as LogosLexer, Logos};

fn skip_line_comment<'src>(lexer: &mut LogosLexer<'src, Token<'src>>) {
    let remainder = lexer.remainder();
    let mut chars = remainder.char_indices().peekable();

    while chars.next_if(|&(_, c)| c != '\n').is_some() {}

    let bytes_skipped = chars.peek().map_or(remainder.len(), |&(pos, _)| pos);
    lexer.bump(bytes_skipped);
}

#[derive(Debug, Clone, Copy, Logos)]
#[logos(skip r"[ \t\f\r]+")]
#[logos(skip(r"//", skip_line_comment))]
pub enum Token<'src> {
    #[token("=")]
    Equal,
    #[regex(r"[\$△]")]
    Triangle,
    #[token("(")]
    Open,
    #[token(")")]
    Close,
    #[token("\n")]
    Newline,
    #[regex("[a-zA-Z0-9_]+")]
    Identifier(&'src str),
    #[token("->")]
    Arrow,
    #[token("fn")]
    Fn,
    #[token("#num")]
    Num,
    #[token("#show")]
    Show,
    #[token("#viz")]
    Visualize,

    Error,
    Eof,
}

pub type Span = Range<usize>;

pub struct Lexer<'src> {
    source: &'src str,
    lexer: LogosLexer<'src, Token<'src>>,
    peeked: Option<(Token<'src>, Span)>,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            lexer: Token::lexer(source),
            peeked: None,
        }
    }

    pub fn next(&mut self) -> (Token<'src>, Span) {
        if let Some(inner) = self.peeked.take() {
            return inner;
        }

        if let Some(tok) = self.lexer.next() {
            return (tok.unwrap_or(Token::Error), self.lexer.span());
        }

        (Token::Eof, self.source.len()..self.source.len())
    }

    pub fn peek(&mut self) -> (Token<'src>, Span) {
        let next = self.next();
        self.peeked = Some(next.clone());
        next
    }

    pub fn next_skip_newline(&mut self) -> (Token<'src>, Span) {
        loop {
            match self.next() {
                (Token::Newline, _) => {}
                (tok, span) => return (tok, span),
            }
        }
    }

    pub fn peek_skip_newline(&mut self) -> (Token<'src>, Span) {
        let next = self.next_skip_newline();
        self.peeked = Some(next.clone());
        next
    }
}
