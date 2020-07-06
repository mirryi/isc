use crate::class::{CharClass, CharRange};
use std::error;
use std::fmt;
use std::result;

pub type Result<T> = result::Result<T, ParseError>;

pub trait Parser<T>
where
    T: Clone,
{
    fn shift_action(
        &self,
        stack: &mut Vec<T>,
        op_stack: &mut Vec<Operator>,
        c: CharClass,
    ) -> Result<()>;

    fn reduce_action(&self, stack: &mut Vec<T>, op_stack: &mut Vec<Operator>) -> Result<()>;

    fn parse(&self, expr: &str) -> Result<Option<T>> {
        let mut state = ParserState::new(
            |stack, op_stack, c| self.shift_action(stack, op_stack, c),
            |stack, op_stack| self.reduce_action(stack, op_stack),
        );

        let mut chars = expr.chars();
        let mut next = chars.next();
        while next.is_some() {
            let c = next.unwrap();

            match c {
                '|' => {
                    if state.escaped {
                        state.escaped = false;
                        if state.in_char_class {
                            // If escaped and in char class, push to char range buffer.
                            state.append_char_range_buf(c);
                        } else {
                            // If escaped and not in char class, handle this as literal |.
                            state.handle_literal_char(c)?;
                        }
                    } else if state.in_char_class {
                        // If not escaped and in char class, push to char range buffer.
                        state.append_char_range_buf(c);
                    } else {
                        // If not escaped and not in char class, handle this as union operator.
                        state.handle_union()?;
                    }
                }
                '*' => {
                    if state.escaped {
                        state.escaped = false;
                        if state.in_char_class {
                            // If escaped and in char class, push to char range buffer.
                            state.append_char_range_buf(c);
                        } else {
                            // If escaped and not in char class, handle this as literal |
                            state.handle_literal_char(c)?;
                        }
                    } else if state.in_char_class {
                        // If not escaped and in char class, push to char range buffer.
                        state.append_char_range_buf(c);
                    } else {
                        // If not escaped, handle this as kleene star operator.
                        state.handle_kleene_star()?;
                    }
                }
                '+' => {
                    if state.escaped {
                        state.escaped = false;
                        if state.in_char_class {
                            // If escaped and in char class, push to char range buffer.
                            state.append_char_range_buf(c);
                        } else {
                            // If escaped and not in char class, handle this as literal +.
                            state.handle_literal_char(c)?;
                        }
                    } else if state.in_char_class {
                        // If not escaped and in char class, push to char range buffer.
                        state.append_char_range_buf(c);
                    } else {
                        // If not escaped and not in char class, handle this as plus operator.
                        state.handle_plus()?;
                    }
                }
                '?' => {
                    if state.escaped {
                        state.escaped = false;
                        if state.in_char_class {
                            // If escaped and in char class, push to char range buffer.
                            state.append_char_range_buf(c);
                        } else {
                            // If escaped and not in char class, handle this as literal ?.
                            state.handle_literal_char(c)?;
                        }
                    } else if state.in_char_class {
                        // If not escaped and in char class, push to char range buffer.
                        state.append_char_range_buf(c);
                    } else {
                        // If not escaped and not in char class, handle this as optional operator.
                        state.handle_optional()?;
                    }
                }
                '(' => {
                    if state.escaped {
                        state.escaped = false;
                        if state.in_char_class {
                            // If escaped and in char class, push to char range buffer.
                            state.append_char_range_buf(c);
                        } else {
                            // If escaped, handle this as literal (.
                            state.handle_literal_char(c)?;
                        }
                    } else if state.in_char_class {
                        // If not escaped and in char class, push to char range buffer.
                        state.append_char_range_buf(c);
                    } else {
                        // If not escaped, handle this as left parentheses
                        state.handle_left_paren()?;
                    }
                }
                ')' => {
                    if state.escaped {
                        state.escaped = false;
                        if state.in_char_class {
                            // If escaped and in char class, push to char range buffer.
                            state.append_char_range_buf(c);
                        } else {
                            // If escaped, handle this as literal |
                            state.handle_literal_char(c)?;
                        }
                    } else if state.in_char_class {
                        // If not escaped and in char class, push to char range buffer.
                        state.append_char_range_buf(c);
                    } else {
                        // If not escaped, handle this as left parentheses
                        state.handle_right_paren()?;
                    }
                }
                '[' => {
                    if state.in_char_class {
                        // Set [ in char class if currently within brackets.
                        state.append_char_range_buf(c);
                    } else if state.escaped {
                        // Handle [ as literal if escaped and not in char class.
                        state.escaped = false;
                        state.handle_literal_char(c)?;
                    } else {
                        // Enter char class until ] is seen if not currently in char class or
                        // escaped.
                        state.in_char_class = true;
                        state.clear_char_class_buf();
                    }
                }
                ']' => {
                    if state.escaped {
                        state.escaped = false;
                        if state.in_char_class {
                            // Handle ] as part in char class if escaped and in char class.
                            state.append_char_range_buf(c);
                        } else {
                            // Handle ] as literal if escaped and not in char class.
                            state.handle_literal_char(c)?;
                        }
                    } else if state.in_char_class {
                        state.handle_right_bracket()?;
                    } else {
                        // Handle ] as literal if not escaped or in char class.
                        state.handle_literal_char(c)?;
                    }
                }
                '\\' => {
                    if state.escaped {
                        // If escaped, handle this as literal \
                        state.escaped = false;
                        state.handle_literal_char(c)?;
                    } else if state.in_char_class {
                        // If unescaped and in char class, handle next.
                        state.escaped = true;
                    } else {
                        // If unescaped and not in char class, handle next.
                        state.escaped = true;
                    }
                }
                '^' => {
                    if state.escaped {
                        state.escaped = false;
                        if state.in_char_class {
                            // If escaped and in char class, handle this as literal ^ in char
                            // class.
                            state.append_char_range_buf(c);
                        } else {
                            // If escaped but not in char class, handle this literal ^.
                            state.handle_literal_char(c)?;
                        }
                    } else if state.in_char_class {
                        // If unescaped and in char class, check if this is the first char in the
                        // character class. If so, set flag to negate the current character class
                        // when shifted.
                        if state.char_range_buf.is_empty()
                            && state.char_class_buf.0.ranges.is_empty()
                        {
                            state.char_class_buf.1 = true;
                        } else {
                            // Otherwise push this as regular char to char class.
                            state.append_char_range_buf(c);
                        }
                    } else {
                        // If unescaped and not in char class, handle this as literal ^.
                        state.handle_literal_char(c)?;
                    }
                }
                '.' => {
                    if state.escaped {
                        state.escaped = false;
                        if state.in_char_class {
                            // If escaped and in char class, handle this as a literal . in char class.
                            state.append_char_range_buf(c);
                        } else {
                            // If escaped and not in char class, handle this as a literal .
                            state.handle_literal_char(c)?;
                        }
                    } else if state.in_char_class {
                        // If unescaped and in char class, push . to char range buf as literal.
                        state.append_char_range_buf(c);
                    } else {
                        // If unescaped and not in char class, add ranges for all chars except \n to
                        // char class buf.
                        let cc = CharClass::all_but_newline();
                        state.handle_char_class(cc)?;
                    }
                }
                _ => {
                    // Kinda spaghetti:
                    let mut is_special = true;
                    let mut cc = CharClass::new();
                    if state.escaped {
                        state.escaped = false;
                        // If sequence is \d,
                        if c == 'd' {
                            cc = CharClass::decimal_number();
                        } else if c == 'D' {
                            cc = CharClass::decimal_number().complement();
                        } else if c == 'w' {
                            cc = CharClass::word();
                        } else if c == 'W' {
                            cc = CharClass::word().complement();
                        } else if c == 'n' {
                            cc = CharClass::new_single('\n');
                        } else if c == 's' {
                            cc = CharClass::whitespace();
                        } else if c == 'S' {
                            cc = CharClass::whitespace().complement();
                        } else {
                            is_special = false;
                        }
                    } else {
                        is_special = false;
                    }

                    if is_special {
                        if state.in_char_class {
                            state.handle_incomplete_char_range_buf();
                            CharClass::copy_into(&mut state.char_class_buf.0, &cc);
                        } else {
                            state.handle_char_class(cc)?;
                        }
                    } else if state.in_char_class {
                        // If in char class, push char to range buffer.
                        state.append_char_range_buf(c);
                    } else {
                        // If not in char class, handle as literal.
                        state.handle_literal_char(c)?;
                    }
                }
            }

            next = chars.next();
        }

        if expr.len() == 0 {
            state.op_stack.push(Operator::EmptyPlaceholder);
        }

        while !state.op_stack.is_empty() {
            state.reduce_stack()?;
        }

        let head = state.stack.into_iter().last();
        Ok(head)
    }
}

#[derive(Debug, PartialEq)]
pub enum Operator {
    Union,
    Concatenation,
    KleeneStar,
    Plus,
    Optional,
    LeftParen,
    EmptyPlaceholder,
}

#[derive(Debug)]
struct ParserState<T, SF, RF>
where
    SF: Copy + FnMut(&mut Vec<T>, &mut Vec<Operator>, CharClass) -> Result<()>,
    RF: Copy + FnMut(&mut Vec<T>, &mut Vec<Operator>) -> Result<()>,
{
    stack: Vec<T>,
    op_stack: Vec<Operator>,
    paren_count_stack: Vec<usize>,

    escaped: bool,
    insert_concat: bool,

    in_char_class: bool,
    char_class_buf: (CharClass, bool),
    char_range_buf: CharRangeBuf,

    shift_action: SF,
    reduce_action: RF,
}

#[derive(Debug)]
struct CharRangeBuf(Option<char>, Option<char>, Option<char>);

impl CharRangeBuf {
    fn new() -> Self {
        CharRangeBuf(None, None, None)
    }

    fn is_empty(&self) -> bool {
        self.0 == None
    }

    fn clear(&mut self) {
        self.0 = None;
        self.1 = None;
        self.2 = None;
    }
}

impl<T, SF, RF> ParserState<T, SF, RF>
where
    SF: Copy + FnMut(&mut Vec<T>, &mut Vec<Operator>, CharClass) -> Result<()>,
    RF: Copy + FnMut(&mut Vec<T>, &mut Vec<Operator>) -> Result<()>,
{
    fn new(shift_action: SF, reduce_action: RF) -> Self {
        Self {
            stack: Vec::new(),
            op_stack: Vec::new(),
            paren_count_stack: Vec::new(),

            escaped: false,
            insert_concat: false,

            in_char_class: false,
            char_class_buf: (CharClass::new(), false),
            char_range_buf: CharRangeBuf::new(),

            shift_action,
            reduce_action,
        }
    }

    fn handle_literal_char(&mut self, c: char) -> Result<()> {
        let char_class = CharClass::new_single(c);
        self.handle_char_class(char_class)
    }

    fn handle_char_class(&mut self, c: CharClass) -> Result<()> {
        while self.precedence_reduce_stack(&Operator::Concatenation)? {}

        if self.insert_concat {
            self.push_operator(Operator::Concatenation);
        }

        self.shift_action(c)?;
        self.insert_concat = true;

        Ok(())
    }

    fn handle_union(&mut self) -> Result<()> {
        let op = Operator::Union;
        self.precedence_reduce_stack(&op)?;

        self.op_stack.push(op);
        self.insert_concat = false;

        Ok(())
    }

    fn handle_kleene_star(&mut self) -> Result<()> {
        let op = Operator::KleeneStar;
        self.precedence_reduce_stack(&op)?;

        self.op_stack.push(op);
        self.insert_concat = true;

        Ok(())
    }

    fn handle_plus(&mut self) -> Result<()> {
        let op = Operator::Plus;
        self.precedence_reduce_stack(&op)?;

        self.op_stack.push(op);
        self.insert_concat = true;

        Ok(())
    }

    fn handle_optional(&mut self) -> Result<()> {
        let op = Operator::Optional;
        self.precedence_reduce_stack(&op)?;

        self.op_stack.push(op);
        self.insert_concat = true;

        Ok(())
    }

    fn handle_left_paren(&mut self) -> Result<()> {
        let op = Operator::LeftParen;
        self.precedence_reduce_stack(&op)?;

        if self.insert_concat {
            self.push_concatenation();
        }

        self.op_stack.push(op);
        self.paren_count_stack.push(self.stack.len());
        self.insert_concat = false;

        Ok(())
    }

    fn handle_right_paren(&mut self) -> Result<()> {
        let last_op = self
            .op_stack
            .last()
            .ok_or(ParseError::UnbalancedOperators)?;
        let prev_node_count = self
            .paren_count_stack
            .last()
            .ok_or(ParseError::UnbalancedParentheses)?;

        if *last_op == Operator::LeftParen && *prev_node_count == self.stack.len() {
            self.op_stack.pop().ok_or(ParseError::UnbalancedOperators)?;
            self.op_stack.push(Operator::EmptyPlaceholder);
            self.reduce_stack()?;
        } else {
            while !self.op_stack.is_empty() && *self.op_stack.last().unwrap() != Operator::LeftParen
            {
                self.reduce_stack()?;
            }
            self.op_stack.pop().ok_or(ParseError::UnbalancedOperators)?;
        }

        self.insert_concat = true;

        Ok(())
    }

    fn handle_right_bracket(&mut self) -> Result<()> {
        // End char class if not escaped and in char class.
        self.in_char_class = false;

        // Throw error if nothing specified between brackets.
        if self.char_range_buf.is_empty() && self.char_class_buf.0.ranges.is_empty() {
            return Err(ParseError::EmptyCharacterClass);
        }

        self.handle_incomplete_char_range_buf();

        // Call shift action on completed char class.
        let char_class = if self.char_class_buf.1 {
            self.char_class_buf.0.complement()
        } else {
            self.char_class_buf.0.clone()
        };
        self.handle_char_class(char_class)?;

        // Clear the char class buffer.
        self.clear_char_class_buf();

        Ok(())
    }

    fn handle_incomplete_char_range_buf(&mut self) {
        // Existing chars in first and second spots of buffer are added to
        // char class as single-char ranges.
        let s0 = self.char_range_buf.0;
        if let Some(s) = s0 {
            self.char_class_buf.0.add_range(CharRange::new_single(s));
            let s1 = self.char_range_buf.1;
            if let Some(s) = s1 {
                self.char_class_buf.0.add_range(CharRange::new_single(s));
            }
        }

        // Clear the char range buffer.
        self.char_range_buf.clear();
    }

    /// This method should only be called when in_char_class is true.
    /// The escaping of character class metasymbols (]) should be handled outside of this method
    /// call.
    fn append_char_range_buf(&mut self, c: char) {
        if self.char_range_buf.0 == None {
            // If first spot is empty, add this char as the start of the range.
            self.char_range_buf.0 = Some(c);
        } else if self.char_range_buf.1 == None {
            if c == '-' {
                // If second spot is empty and this char is a dash, fill second spot.
                self.char_range_buf.1 = Some(c);
            } else {
                // If second spot is empty but this char is not a dash, add a single-char range to
                // the char class buffer.
                let new_range_char = self.char_range_buf.0.unwrap();
                let new_range = CharRange::new_single(new_range_char);
                self.char_class_buf.0.add_range(new_range);

                // Clear the range buffer.
                self.char_range_buf.clear();

                // Retry appending this char.
                self.append_char_range_buf(c);
            }
        } else if self.char_range_buf.2 == None {
            // If third spot is empty, complete the range and add it to the char class buffer.
            let start = self.char_range_buf.0.unwrap();
            let end = c;
            let new_range = CharRange::new(start, end);
            self.char_class_buf.0.add_range(new_range);

            self.char_range_buf.clear();
        }
        // There should never be a situation where all spots are filled.
    }

    fn clear_char_class_buf(&mut self) {
        self.char_class_buf = (CharClass::new(), false);
    }

    fn reduce_stack(&mut self) -> Result<()> {
        self.reduce_action()
    }

    fn precedence_reduce_stack(&mut self, op: &Operator) -> Result<bool> {
        let reduce = match self.op_stack.last() {
            Some(last_op) => {
                if last_op == op && *last_op != Operator::LeftParen {
                    // If current op is the same as last, collapse the last.
                    // If both of left parenthesis, do nothing
                    true
                } else if *op == Operator::Union {
                    // If current op is alternation, collapse last if it is concat, kleene, plus,
                    // or optional.
                    *last_op == Operator::Concatenation
                        || *last_op == Operator::KleeneStar
                        || *last_op == Operator::Plus
                        || *last_op == Operator::Optional
                } else if *op == Operator::Concatenation {
                    // If current op is concat, collapse last if it is kleene, plus, or optional.
                    *last_op == Operator::KleeneStar
                        || *last_op == Operator::Plus
                        || *last_op == Operator::Optional
                } else if *op == Operator::KleeneStar
                    || *op == Operator::Plus
                    || *op == Operator::Optional
                {
                    // If current op is kleene star, plus, or optional, do not collapse last
                    // because they are highest precedence.
                    false
                } else if *op == Operator::LeftParen {
                    // If current op is left parenthesis, collapse last if it is kleene star, plus,
                    // or optional, which operate only on left node.
                    *last_op == Operator::KleeneStar
                        || *last_op == Operator::Plus
                        || *last_op == Operator::Optional
                } else {
                    false
                }
            }
            None => false,
        };

        if reduce {
            self.reduce_stack()?;
        }

        Ok(reduce)
    }

    fn push_operator(&mut self, op: Operator) {
        self.op_stack.push(op);
    }

    fn push_concatenation(&mut self) {
        self.op_stack.push(Operator::Concatenation);
    }

    fn shift_action(&mut self, c: CharClass) -> Result<()> {
        (self.shift_action)(&mut self.stack, &mut self.op_stack, c)
    }

    fn reduce_action(&mut self) -> Result<()> {
        (self.reduce_action)(&mut self.stack, &mut self.op_stack)
    }
}

#[derive(Debug)]
pub enum ParseError {
    UnbalancedOperators,
    UnbalancedParentheses,
    EmptyCharacterClass,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::UnbalancedOperators => write!(f, "unbalanced operators"),
            Self::UnbalancedParentheses => write!(f, "unbalanced parentheses"),
            Self::EmptyCharacterClass => write!(f, "empty character class"),
        }
    }
}

impl error::Error for ParseError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            _ => None,
        }
    }
}