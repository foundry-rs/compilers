use std::{fmt, fmt::Write, iter::Peekable, str::CharIndices};

type Spanned<Token, Loc, Error> = Result<(Token, Loc), Error>;

macro_rules! syntax_err {
    ($msg:expr) => {{
        Err(SyntaxError::new($msg))
    }};
    ($msg:expr, $($tt:tt)*) => {{
        Err(SyntaxError::new(format!($msg, $($tt)*)))
    }};
}

/// An error that can happen during source map parsing.
#[derive(Clone, Debug, thiserror::Error)]
#[error("{0}")]
pub struct SyntaxError(String);

impl SyntaxError {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl From<std::num::TryFromIntError> for SyntaxError {
    fn from(_value: std::num::TryFromIntError) -> Self {
        Self::new("offset overflow".to_string())
    }
}

#[derive(PartialEq, Eq)]
enum Token<'a> {
    Number(&'a str),
    Semicolon,
    Colon,
    /// `i` which represents an instruction that goes into a function
    In,
    /// `o` which represents an instruction that returns from a function
    Out,
    /// `-` regular jump
    Regular,
}

impl<'a> fmt::Debug for Token<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Number(s) => write!(f, "NUMBER({s:?})"),
            Token::Semicolon => write!(f, "SEMICOLON"),
            Token::Colon => write!(f, "COLON"),
            Token::In => write!(f, "JMP(i)"),
            Token::Out => write!(f, "JMP(o)"),
            Token::Regular => write!(f, "JMP(-)"),
        }
    }
}

impl<'a> fmt::Display for Token<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Number(_) => write!(f, "number"),
            Token::Semicolon => write!(f, "`;`"),
            Token::Colon => write!(f, "`:`"),
            Token::In => write!(f, "jmp-in"),
            Token::Out => write!(f, "jmp-out"),
            Token::Regular => write!(f, "jmp"),
        }
    }
}

struct TokenStream<'input> {
    input: &'input str,
    chars: Peekable<CharIndices<'input>>,
}

impl<'input> TokenStream<'input> {
    pub fn new(input: &'input str) -> Self {
        TokenStream { chars: input.char_indices().peekable(), input }
    }

    fn number(
        &mut self,
        start: usize,
        mut end: usize,
    ) -> Option<Spanned<Token<'input>, usize, SyntaxError>> {
        loop {
            if let Some((_, ch)) = self.chars.peek().cloned() {
                if !ch.is_ascii_digit() {
                    break;
                }
                self.chars.next();
                end += 1;
            } else {
                end = self.input.len();
                break;
            }
        }
        Some(Ok((Token::Number(&self.input[start..end]), start)))
    }
}

impl<'input> Iterator for TokenStream<'input> {
    type Item = Spanned<Token<'input>, usize, SyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.chars.next()? {
            (i, ';') => Some(Ok((Token::Semicolon, i))),
            (i, ':') => Some(Ok((Token::Colon, i))),
            (i, 'i') => Some(Ok((Token::In, i))),
            (i, 'o') => Some(Ok((Token::Out, i))),
            (start, '-') => match self.chars.peek() {
                Some((_, ch)) if ch.is_ascii_digit() => {
                    self.chars.next();
                    self.number(start, start + 2)
                }
                _ => Some(Ok((Token::Regular, start))),
            },
            (start, ch) if ch.is_ascii_digit() => self.number(start, start + 1),
            (i, c) => Some(syntax_err!("Unexpected input {} at {}", c, i)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Jump {
    /// A jump instruction that goes into a function
    In,
    /// A jump  represents an instruction that returns from a function
    Out,
    /// A regular jump instruction
    Regular,
}

impl Jump {
    fn to_int(self) -> u32 {
        match self {
            Self::In => 0,
            Self::Out => 1,
            Self::Regular => 2,
        }
    }

    fn from_int(i: u32) -> Self {
        match i {
            0 => Self::In,
            1 => Self::Out,
            2 => Self::Regular,
            _ => unreachable!(),
        }
    }
}

impl AsRef<str> for Jump {
    fn as_ref(&self) -> &str {
        match self {
            Self::In => "i",
            Self::Out => "o",
            Self::Regular => "-",
        }
    }
}

impl fmt::Display for Jump {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

/// A Solidity source map, which is composed of multiple [`SourceElement`]s, separated by
/// semicolons.
///
/// Solidity reference: <https://docs.soliditylang.org/en/latest/internals/source_mappings.html#source-mappings>
pub type SourceMap = Vec<SourceElement>;

/// A single element in a [`SourceMap`].
///
/// Solidity reference: <https://docs.soliditylang.org/en/latest/internals/source_mappings.html#source-mappings>
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SourceElement {
    offset: u32,
    length: u32,
    index: i32,
    // 2 bits for jump, 30 bits for modifier depth; see [set_jump_and_modifier_depth]
    jump_and_modifier_depth: u32,
}

impl SourceElement {
    /// Creates a new source element with default values.
    pub fn new_invalid() -> Self {
        Self { offset: 0, length: 0, index: -1, jump_and_modifier_depth: 0 }
    }

    /// The byte-offset to the start of the range in the source file.
    #[inline]
    pub fn offset(&self) -> u32 {
        self.offset
    }

    /// The length of the source range in bytes.
    #[inline]
    pub fn length(&self) -> u32 {
        self.length
    }

    /// The source index.
    ///
    /// Note: In the case of instructions that are not associated with any particular source file,
    /// the source mapping assigns an integer identifier of -1. This may happen for bytecode
    /// sections stemming from compiler-generated inline assembly statements.
    /// This case is represented as a `None` value.
    #[inline]
    pub fn index(&self) -> Option<u32> {
        if self.index == -1 {
            None
        } else {
            Some(self.index as u32)
        }
    }

    /// The source index.
    ///
    /// See [`Self::index`] for more information.
    #[inline]
    pub fn index_i32(&self) -> i32 {
        self.index
    }

    /// Jump instruction.
    #[inline]
    pub fn jump(&self) -> Jump {
        Jump::from_int(self.jump_and_modifier_depth >> 30)
    }

    #[inline]
    fn set_jump(&mut self, jump: Jump) {
        self.set_jump_and_modifier_depth(jump, self.modifier_depth());
    }

    /// Modifier depth.
    ///
    /// This depth is increased whenever the placeholder statement (`_`) is entered in a modifier
    /// and decreased when it is left again.
    #[inline]
    pub fn modifier_depth(&self) -> u32 {
        (self.jump_and_modifier_depth << 2) >> 2
    }

    #[inline]
    fn set_modifier_depth(&mut self, modifier_depth: u32) {
        self.set_jump_and_modifier_depth(self.jump(), modifier_depth);
    }

    #[inline]
    fn set_jump_and_modifier_depth(&mut self, jump: Jump, modifier_depth: u32) {
        self.jump_and_modifier_depth = (jump.to_int() << 30) | modifier_depth;
    }
}

impl fmt::Display for SourceElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}:{}",
            self.offset(),
            self.length(),
            self.index_i32(),
            self.jump(),
            self.modifier_depth(),
        )
    }
}

#[derive(Default)]
struct SourceElementBuilder {
    offset: Option<usize>,
    length: Option<usize>,
    index: Option<Option<u32>>,
    jump: Option<Jump>,
    modifier_depth: Option<usize>,
}

impl fmt::Display for SourceElementBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.offset.is_none()
            && self.length.is_none()
            && self.index.is_none()
            && self.jump.is_none()
            && self.modifier_depth.is_none()
        {
            return Ok(());
        }

        if let Some(s) = self.offset {
            if self.index == Some(None) {
                f.write_str("-1")?;
            } else {
                s.fmt(f)?;
            }
        }
        if self.length.is_none()
            && self.index.is_none()
            && self.jump.is_none()
            && self.modifier_depth.is_none()
        {
            return Ok(());
        }
        f.write_char(':')?;

        if let Some(s) = self.length {
            if self.index == Some(None) {
                f.write_str("-1")?;
            } else {
                s.fmt(f)?;
            }
        }
        if self.index.is_none() && self.jump.is_none() && self.modifier_depth.is_none() {
            return Ok(());
        }
        f.write_char(':')?;

        if let Some(s) = self.index {
            let s = s.map(|s| s as i64).unwrap_or(-1);
            s.fmt(f)?;
        }
        if self.jump.is_none() && self.modifier_depth.is_none() {
            return Ok(());
        }
        f.write_char(':')?;

        if let Some(s) = self.jump {
            s.fmt(f)?;
        }
        if self.modifier_depth.is_none() {
            return Ok(());
        }
        f.write_char(':')?;

        if let Some(s) = self.modifier_depth {
            if self.index == Some(None) {
                f.write_str("-1")?;
            } else {
                s.fmt(f)?;
            }
        }

        Ok(())
    }
}

impl SourceElementBuilder {
    fn finish(self, prev: Option<SourceElement>) -> Result<SourceElement, SyntaxError> {
        let no_prev = prev.is_none();
        let mut element = prev.unwrap_or_else(SourceElement::new_invalid);
        macro_rules! get_field {
            (| $field:ident | $e:expr) => {
                if let Some($field) = self.$field {
                    $e;
                } else if no_prev {
                    return Err(SyntaxError::new(format!("No previous {}", stringify!($field))));
                }
            };
        }
        get_field!(|offset| element.offset = offset.try_into()?);
        get_field!(|length| element.length = length.try_into()?);
        get_field!(|index| element.index = index.map(|x| x as i32).unwrap_or(-1));
        get_field!(|jump| element.set_jump(jump));
        // Modifier depth is optional.
        if let Some(modifier_depth) = self.modifier_depth {
            element.set_modifier_depth(modifier_depth.try_into()?);
        }
        Ok(element)
    }

    fn set_jmp(&mut self, jmp: Jump, i: usize) -> Option<SyntaxError> {
        if self.jump.is_some() {
            return Some(SyntaxError::new(format!("Jump already set: {i}")));
        }
        self.jump = Some(jmp);
        None
    }

    fn set_offset(&mut self, offset: usize, i: usize) -> Option<SyntaxError> {
        if self.offset.is_some() {
            return Some(SyntaxError::new(format!("Offset already set: {i}")));
        }
        self.offset = Some(offset);
        None
    }

    fn set_length(&mut self, length: usize, i: usize) -> Option<SyntaxError> {
        if self.length.is_some() {
            return Some(SyntaxError::new(format!("Length already set: {i}")));
        }
        self.length = Some(length);
        None
    }

    fn set_index(&mut self, index: Option<u32>, i: usize) -> Option<SyntaxError> {
        if self.index.is_some() {
            return Some(SyntaxError::new(format!("Index already set: {i}")));
        }
        self.index = Some(index);
        None
    }

    fn set_modifier(&mut self, modifier_depth: usize, i: usize) -> Option<SyntaxError> {
        if self.modifier_depth.is_some() {
            return Some(SyntaxError::new(format!("Modifier depth already set: {i}")));
        }
        self.modifier_depth = Some(modifier_depth);
        None
    }
}

pub struct Parser<'input> {
    stream: TokenStream<'input>,
    last_element: Option<SourceElement>,
    done: bool,
    #[cfg(test)]
    output: Option<&'input mut dyn Write>,
}

impl<'input> Parser<'input> {
    pub fn new(input: &'input str) -> Self {
        Self {
            stream: TokenStream::new(input),
            last_element: None,
            done: false,
            #[cfg(test)]
            output: None,
        }
    }
}

macro_rules! parse_number {
    ($num:expr, $pos:expr) => {{
        let num = match $num.parse::<i64>() {
            Ok(num) => num,
            Err(_) => {
                return Some(syntax_err!("Expected {} to be a valid integer at {}", $num, $pos))
            }
        };
        match num {
            i if i < -1 => {
                return Some(syntax_err!("Unexpected negative identifier of `{}` at {}", i, $pos))
            }
            -1 => None,
            i => Some(i as u32),
        }
    }};
}

macro_rules! bail_opt {
    ($opt:expr) => {
        if let Some(err) = $opt {
            return Some(Err(err));
        }
    };
}

impl<'input> Iterator for Parser<'input> {
    type Item = Result<SourceElement, SyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        // start parsing at the offset state, `s`
        let mut state = State::Offset;
        let mut builder = SourceElementBuilder::default();

        loop {
            match self.stream.next() {
                Some(Ok((token, pos))) => match token {
                    Token::Semicolon => break,
                    Token::Number(num) => match state {
                        State::Offset => {
                            bail_opt!(builder.set_offset(
                                parse_number!(num, pos).unwrap_or_default() as usize,
                                pos
                            ))
                        }
                        State::Length => {
                            bail_opt!(builder.set_length(
                                parse_number!(num, pos).unwrap_or_default() as usize,
                                pos
                            ))
                        }
                        State::Index => {
                            bail_opt!(builder.set_index(parse_number!(num, pos), pos))
                        }
                        State::Modifier => {
                            bail_opt!(builder.set_modifier(
                                parse_number!(num, pos).unwrap_or_default() as usize,
                                pos
                            ))
                        }
                        State::Jmp => {
                            return Some(syntax_err!("Expected Jump found number at {}", pos))
                        }
                    },
                    Token::Colon => {
                        bail_opt!(state.advance(pos))
                    }
                    Token::In => {
                        bail_opt!(builder.set_jmp(Jump::In, pos))
                    }
                    Token::Out => {
                        bail_opt!(builder.set_jmp(Jump::Out, pos))
                    }
                    Token::Regular => {
                        bail_opt!(builder.set_jmp(Jump::Regular, pos))
                    }
                },
                Some(Err(err)) => return Some(Err(err)),
                None => {
                    if self.done {
                        return None;
                    }
                    self.done = true;
                    break;
                }
            }
        }

        #[cfg(test)]
        if let Some(out) = self.output.as_mut() {
            if self.last_element.is_some() {
                let _ = out.write_char(';');
            }
            let _ = out.write_str(&builder.to_string());
        }

        let element = match builder.finish(self.last_element.take()) {
            Ok(element) => {
                self.last_element = Some(element.clone());
                Ok(element)
            }
            Err(err) => Err(err),
        };
        Some(element)
    }
}

/// State machine to keep track of separating `:`
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    // s
    Offset,
    // l
    Length,
    // f
    Index,
    // j
    Jmp,
    // m
    Modifier,
}

impl State {
    fn advance(&mut self, i: usize) -> Option<SyntaxError> {
        match self {
            Self::Offset => *self = Self::Length,
            Self::Length => *self = Self::Index,
            Self::Index => *self = Self::Jmp,
            Self::Jmp => *self = Self::Modifier,
            Self::Modifier => return Some(SyntaxError::new(format!("unexpected colon at {i}"))),
        }
        None
    }
}

/// Parses a source map
pub fn parse(input: &str) -> Result<SourceMap, SyntaxError> {
    Parser::new(input).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_parse_source_maps() {
        // all source maps from the compiler output test data
        let source_maps = include_str!("../../../../test-data/out-source-maps.txt");

        for (line, s) in source_maps.lines().enumerate() {
            parse(s).unwrap_or_else(|e| panic!("Failed to parse line {line}: {e}"));
        }
    }

    #[test]
    fn can_parse_foundry_cheatcodes_sol_maps() {
        let s = include_str!("../../../../test-data/cheatcodes.sol-sourcemap.txt");
        let mut out = String::new();
        let mut parser = Parser::new(s);
        parser.output = Some(&mut out);
        let _map = parser.collect::<Result<SourceMap, _>>().unwrap();
        assert_eq!(out, s);
    }
}
