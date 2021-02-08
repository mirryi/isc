use super::error::{ExpectedToken, ParseError};
use super::{Parse, ParseInput, Symbol};
use crate::ast::Ident;
use crate::token::Token;

impl<I> Parse<I> for Ident
where
    I: Iterator<Item = Symbol>,
{
    #[inline]
    fn parse(input: &mut ParseInput<I>) -> Result<Self, ()> {
        let next = input.next_unwrap(|| vec![ExpectedToken::Ident])?;
        let name = match next.0 {
            Token::Ident(ident) => ident,
            _ => {
                input.error(unexpectedtoken!(next.1, next.0, ExpectedToken::Ident));
                return Err(());
            }
        };

        Ok(Self { name })
    }
}
