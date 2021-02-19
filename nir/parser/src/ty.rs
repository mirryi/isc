use crate::{ExpectedToken, Parse, ParseError, ParseInput, ParseResult, Symbol};

use ast::{
    keywords::{LBracket, RParen},
    ArrayType, DeclaredType, Ident, PrimitiveType, PrimitiveTypeKind, Span, Spanned, Type,
};
use lexer::Token;

impl<I> Parse<I> for Type
where
    I: Iterator<Item = Symbol>,
{
    #[inline]
    fn parse(input: &mut ParseInput<I>) -> ParseResult<Self> {
        let next = input.next_unwrap(|| {
            vec![
                ExpectedToken::Type,
                ExpectedToken::Ident,
                ereserved!(LParen),
                ereserved!(LBracket),
            ]
        })?;

        let ty = match next.0 {
            Token::Type(ty) => Type::Primitive(from_lexer_type(ty, next.1)),
            // Non-primitive type; check symbol table stack for its existence.
            Token::Ident(name) => {
                if input.sm.lookup(&name).is_none() {
                    input.error(ParseError::UndeclaredType(Ident {
                        name: Spanned::new(name.clone(), next.1.clone()),
                    }));
                }
                Type::Declared(DeclaredType {
                    name: Ident {
                        name: Spanned::new(name, next.1),
                    },
                })
            }
            reserved!(LParen) => {
                input.consume::<RParen>()?;
                Type::Primitive(PrimitiveType {
                    kind: PrimitiveTypeKind::Unit,
                    span: next.1,
                })
            }
            reserved!(LBracket) => Type::Array(Box::new(ArrayType {
                lbracket_t: Spanned::new(LBracket, next.1),
                ty: input.parse()?,
                rbracket_t: input.consume()?,
            })),
            _ => {
                input.error(unexpectedtoken!(
                    next.1,
                    next.0,
                    ExpectedToken::Type,
                    ExpectedToken::Ident,
                    ereserved!(LParen),
                    ereserved!(LBracket)
                ));
                return Err(());
            }
        };

        Ok(ty)
    }
}

fn from_lexer_type(ty: lexer::Type, span: Span) -> PrimitiveType {
    let kind = match ty {
        lexer::Type::Bool => PrimitiveTypeKind::Bool,
        lexer::Type::Char => PrimitiveTypeKind::Char,
        lexer::Type::I8 => PrimitiveTypeKind::I8,
        lexer::Type::I16 => PrimitiveTypeKind::I16,
        lexer::Type::I32 => PrimitiveTypeKind::I32,
        lexer::Type::I64 => PrimitiveTypeKind::I64,
        lexer::Type::I128 => PrimitiveTypeKind::I128,
        lexer::Type::U8 => PrimitiveTypeKind::U8,
        lexer::Type::U16 => PrimitiveTypeKind::U16,
        lexer::Type::U32 => PrimitiveTypeKind::U32,
        lexer::Type::U64 => PrimitiveTypeKind::U64,
        lexer::Type::F32 => PrimitiveTypeKind::F32,
        lexer::Type::F64 => PrimitiveTypeKind::F64,
    };

    PrimitiveType { kind, span }
}
