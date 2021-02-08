#[macro_use]
pub mod error;

mod ident;
mod item;
mod program;
mod structs;
mod visibility;

pub use error::ParseError;

use self::error::ExpectedToken;
use crate::ast::{Program, Spanned};
use crate::token::{Reserved, ReservedVariant, Token};

use std::iter::Peekable;

pub type Result<T> = std::result::Result<T, Vec<ParseError>>;
pub type ParseResult<T> = std::result::Result<T, ()>;

pub trait Parse<I>
where
    I: Iterator<Item = Symbol>,
    Self: Sized,
{
    fn parse(input: &mut ParseInput<I>) -> ParseResult<Self>;
}

pub type Symbol = Spanned<Token>;

#[derive(Debug)]
pub struct ParseInput<I>
where
    I: Iterator<Item = Symbol>,
{
    pub inner: Peekable<I>,
    pub errors: Vec<ParseError>,
}

impl<I> ParseInput<I>
where
    I: Iterator<Item = Symbol>,
{
    #[inline]
    pub fn new(inner: I) -> Self {
        Self {
            inner: inner.peekable(),
            errors: Vec::new(),
        }
    }

    #[inline]
    pub fn parse<T>(&mut self) -> ParseResult<T>
    where
        T: Parse<I>,
    {
        T::parse(self)
    }

    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Symbol> {
        self.inner.next()
    }

    #[inline]
    pub fn next_unwrap<F>(&mut self, expected: F) -> ParseResult<Symbol>
    where
        F: Fn() -> Vec<ExpectedToken>,
    {
        self.next()
            .ok_or_else(|| self.error(ParseError::UnexpectedEof(expected())))
    }

    #[inline]
    pub fn next_checked<F>(&mut self, check: &Token, expected: F) -> ParseResult<Symbol>
    where
        F: Fn() -> Vec<ExpectedToken>,
    {
        match self.next() {
            Some(sym) if sym.0 == *check => Ok(sym),
            _ => {
                self.error(ParseError::UnexpectedEof(expected()));
                Err(())
            }
        }
    }

    #[inline]
    pub fn peek(&mut self) -> Option<&Symbol> {
        self.inner.peek()
    }

    #[inline]
    pub fn peek_is(&mut self, expected: &Token) -> bool {
        match self.peek() {
            Some(peeked) => peeked.0 == *expected,
            None => false,
        }
    }

    #[inline]
    pub fn is_empty(&mut self) -> bool {
        self.inner.peek().is_none()
    }

    #[inline]
    pub fn error(&mut self, error: ParseError) {
        self.errors.push(error)
    }

    #[inline]
    pub fn consume<R: ReservedVariant>(&mut self) -> ParseResult<()> {
        self.next_checked(&Token::Reserved(R::variant()), || {
            vec![ExpectedToken::Reserved(R::variant())]
        })?;
        Ok(())
    }

    #[inline]
    pub fn consume_opt<R: ReservedVariant>(&mut self) -> ParseResult<Option<()>> {
        match self.next() {
            Some(next) if next == Token::Reserved(R::variant()) => Ok(Some(())),
            None => Ok(None),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct Parser {}

impl Parser {
    #[inline]
    pub const fn new() -> Self {
        Self {}
    }

    /// Parse the input tokens into a syntax tree.
    #[inline]
    pub fn parse<I>(&self, input: I) -> Result<Program>
    where
        I: Iterator<Item = Symbol>,
    {
        let mut input = ParseInput::new(input);
        input.parse().map_err(|_| input.errors)
    }
}
