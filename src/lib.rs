use std::{fmt::Debug, iter::Peekable};

use log::{debug, trace, warn};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unexpected end of input")]
    EOF,
    #[error("Unknown character class `\\{0}`")]
    UnknownCharacterType(char),
    #[error("Unterminated group")]
    UnterminatedGroup,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SingleCharacterMatcher {
    Literal(char),
    Any,
    Digit,
    Alphanumeric,
    Group(Vec<SingleCharacterMatcher>),
    NegativeGroup(Vec<SingleCharacterMatcher>),
}

impl SingleCharacterMatcher {
    pub fn new(input: &mut Peekable<impl Iterator<Item = char>>) -> Result<Self> {
        match input.next() {
            Some('\\') => Self::new_class(input.next().ok_or(Error::EOF)?),
            Some('[') => Self::new_group(input),
            Some('.') => Ok(Self::Any),
            Some(ch) => Ok(Self::new_literal(ch)),
            None => Err(Error::EOF),
        }
    }

    fn new_in_group(input: &mut impl Iterator<Item = char>) -> Result<Self> {
        match input.next() {
            Some('\\') => Self::new_class(input.next().ok_or(Error::EOF)?),
            Some(ch) => Ok(Self::new_literal(ch)),
            None => Err(Error::EOF),
        }
    }

    pub fn new_literal(ch: char) -> Self {
        SingleCharacterMatcher::Literal(ch)
    }

    pub fn new_class(class: char) -> Result<Self> {
        match class {
            ch if !ch.is_alphanumeric() => Ok(Self::Literal(ch)),
            'd' => Ok(Self::Digit),
            'w' => Ok(Self::Alphanumeric),
            ch => Err(Error::UnknownCharacterType(ch)),
        }
    }

    pub fn new_group(input: &mut Peekable<impl Iterator<Item = char>>) -> Result<Self> {
        let mut options = Vec::new();
        let negative = if input.peek() == Some(&'^') {
            input.next(); // Consume "^"
            true
        } else {
            false
        };

        while let Some(ch) = input.peek() {
            if *ch == ']' {
                input.next(); // Consume the ']' character
                if negative {
                    return Ok(Self::NegativeGroup(options));
                } else {
                    return Ok(Self::Group(options));
                }
            } else {
                options.push(Self::new_in_group(input)?);
            }
        }

        Err(Error::EOF)
    }

    pub fn test(&self, ch: char) -> bool {
        match self {
            SingleCharacterMatcher::Literal(c) => *c == ch,
            SingleCharacterMatcher::Digit => ch.is_ascii_digit(),
            SingleCharacterMatcher::Alphanumeric => ch.is_ascii_alphanumeric() || ch == '_',
            SingleCharacterMatcher::Group(options) => options.iter().any(|o| o.test(ch)),
            SingleCharacterMatcher::NegativeGroup(options) => !options.iter().any(|o| o.test(ch)),
            SingleCharacterMatcher::Any => true,
        }
    }
}

#[derive(Debug, Clone)]
struct GroupBacktrackState<'a, T> {
    input: BufferedIterator<T>,
    captures: Vec<String>,
    backtrack: BacktrackInfo<'a, T>,
    matchers_to_resume: std::slice::Iter<'a, Matcher>,
}

#[derive(Debug, Clone)]
enum BacktrackInfo<'a, T> {
    Range(usize),
    Group(usize, Vec<GroupBacktrackState<'a, T>>),
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Matcher {
    Repeat {
        matcher: Box<Matcher>,
        min: Option<usize>,
        max: Option<usize>,
    },
    CaptureGroup(Vec<Matcher>),
    SingleCharacter(SingleCharacterMatcher),
    Backreference(usize),
    StartOfString,
    EndOfString,
    Alternative,
}

#[derive(Debug, Clone)]
struct BufferedIterator<T> {
    inner: T,
    buffer: String,
    subbuffers: Vec<String>,
    peeked: Option<Option<(usize, char)>>,
}

impl<T: Iterator<Item = (usize, char)>> BufferedIterator<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            buffer: String::new(),
            subbuffers: Vec::new(),
            peeked: None,
        }
    }

    pub fn subdivide(&mut self) {
        self.subbuffers.push(String::new());
    }

    pub fn pop_divided(&mut self) -> Option<String> {
        self.subbuffers.pop()
    }

    pub fn peek(&mut self) -> Option<&(usize, char)> {
        let iter = &mut self.inner;
        self.peeked.get_or_insert_with(|| iter.next()).as_ref()
    }

    pub fn take(self) -> (T, String) {
        (self.inner, self.buffer)
    }
}

impl<T: Iterator<Item = (usize, char)>> Iterator for BufferedIterator<T> {
    type Item = (usize, char);

    fn next(&mut self) -> Option<Self::Item> {
        let ret = match self.peeked.take() {
            Some(ch) => ch,
            None => self.inner.next(),
        };

        if let Some((_, ch)) = ret {
            self.buffer.push(ch);
            for subbuffer in self.subbuffers.iter_mut() {
                subbuffer.push(ch);
            }
        }

        ret
    }
}

impl Matcher {
    pub fn new(input: &mut Peekable<impl Iterator<Item = char> + Clone>) -> Result<Self> {
        match input.peek() {
            Some('^') => {
                input.next();
                Ok(Self::StartOfString)
            }
            Some('$') => {
                input.next();
                Ok(Self::EndOfString)
            }
            Some('(') => {
                input.next();
                let mut matchers = Vec::new();
                let mut terminated = false;
                while let Some(ch) = input.peek() {
                    if *ch == ')' {
                        input.next();
                        terminated = true;
                        break;
                    }
                    matchers.push(Matcher::new(input)?);
                }
                if !terminated {
                    return Err(Error::UnterminatedGroup);
                }
                Ok(Self::maybe_repeat(Self::CaptureGroup(matchers), input))
            }
            Some('|') => {
                input.next();
                Ok(Self::Alternative)
            }
            Some(_) => {
                let mut cloned_iter = input.clone();
                if let (Some('\\'), Some(dig)) = (cloned_iter.next(), cloned_iter.next()) {
                    if dig.is_ascii_digit() {
                        std::mem::swap(input, &mut cloned_iter);
                        return Ok(Self::Backreference(String::from(dig).parse().expect(
                            "Dig was checked to be a digit, parsing as usize should pass",
                        )));
                    }
                }
                let matcher = Self::SingleCharacter(SingleCharacterMatcher::new(input)?);

                Ok(Self::maybe_repeat(matcher, input))
            }
            None => Err(Error::EOF),
        }
    }

    fn maybe_repeat(matcher: Matcher, input: &mut Peekable<impl Iterator<Item = char>>) -> Matcher {
        match input.peek() {
            Some('+') => {
                input.next();
                Self::Repeat {
                    matcher: Box::new(matcher),
                    min: Some(1),
                    max: None,
                }
            }
            Some('*') => {
                input.next();
                Self::Repeat {
                    matcher: Box::new(matcher),
                    min: None,
                    max: None,
                }
            }
            Some('?') => {
                input.next();
                Self::Repeat {
                    matcher: Box::new(matcher),
                    min: None,
                    max: Some(1),
                }
            }
            _ => matcher,
        }
    }

    pub fn test<'a, T>(
        &'a self,
        input: &mut BufferedIterator<T>,
        captured_groups: &Vec<String>,
        backtrack: Option<BacktrackInfo<'a, T>>,
    ) -> (bool, Vec<String>, Option<BacktrackInfo<'a, T>>)
    where
        T: Iterator<Item = (usize, char)> + Clone + Debug,
    {
        match self {
            Matcher::SingleCharacter(c) => (
                input.next().is_some_and(|ch| c.test(ch.1)),
                Vec::new(),
                None,
            ),
            Matcher::StartOfString => (
                input.peek().is_some_and(|(idx, _)| *idx == 0),
                Vec::new(),
                None,
            ),
            Matcher::EndOfString => (input.peek().is_none(), Vec::new(), None),
            Matcher::CaptureGroup(inner) => {
                Self::test_group(inner, input, captured_groups, backtrack)
            }
            Matcher::Backreference(index) => {
                let index = index - 0;
                if index >= captured_groups.len() {
                    warn!("Referenced nonexistent group {index}. Captured: {captured_groups:?}");
                    (false, Vec::new(), None)
                } else {
                    debug!(
                        "Backreference {index}: '{:?}' - '{:?}'",
                        captured_groups,
                        Iterator::take(input.clone(), captured_groups[index].len())
                            .collect::<Vec<_>>()
                    );
                    (
                        captured_groups[index]
                            .chars()
                            .all(|ch| input.next().is_some_and(|c| c.1 == ch)),
                        Vec::new(),
                        None,
                    )
                }
            }
            Matcher::Alternative => todo!("Alternatives are only supported in capture groups"),
            Matcher::Repeat { matcher, min, max } => {
                let mut count = 0;
                loop {
                    let mut input_clone = input.clone();
                    if !matcher
                        .test(&mut input_clone, captured_groups, backtrack.clone())
                        .0
                    {
                        break;
                    }
                    if let Some(BacktrackInfo::Range(consumed)) = backtrack {
                        if count >= (consumed - 1) {
                            trace!(
                                "Range limiting due to backtracking {count} >= ({consumed} - 1)"
                            );
                            break;
                        }
                    }
                    std::mem::swap(input, &mut input_clone);
                    count += 1;
                    if let Some(max) = max {
                        if count == *max {
                            trace!("Range limiting due to max {count} == {max}");
                            break;
                        }
                    }
                }

                if let Some(min) = min {
                    if count < *min {
                        debug!("Range failed due to min {count} < {min}");
                        return (false, Vec::new(), None);
                    }
                }

                if count > 0 {
                    return (true, Vec::new(), Some(BacktrackInfo::Range(count)));
                } else {
                    return (true, Vec::new(), None);
                }
            }
        }
    }

    fn test_group<'a, T>(
        inner: &'a Vec<Self>,
        input: &mut BufferedIterator<T>,
        captured_groups: &Vec<String>,
        backtrack: Option<BacktrackInfo<'a, T>>,
    ) -> (bool, Vec<String>, Option<BacktrackInfo<'a, T>>)
    where
        T: Iterator<Item = (usize, char)> + Clone + Debug,
    {
        let options = inner.split(|m| m == &Matcher::Alternative);
        'option_loop: for (option_id, option) in options.enumerate() {
            let mut backtrack_stack: Vec<GroupBacktrackState<'_, T>> =
                if let Some(BacktrackInfo::Group(stack_option, stack)) = backtrack.clone() {
                    if stack_option == option_id {
                        if stack.is_empty() {
                            continue 'option_loop;
                        }
                        stack
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
            let mut backtrack_info = None;
            let mut buffered_input = input.clone();
            let mut our_captures = Vec::new();
            let mut all_captures = captured_groups.clone();
            all_captures.push(String::new()); // Placeholder for our group
            buffered_input.subdivide();

            let mut matcher_iter = option.iter();
            let mut backtrack_matcher_iter = matcher_iter.clone();

            if let Some(mut state) = backtrack_stack.pop() {
                trace!("Backtracking on entry: {:?}, {:?}", buffered_input, state);
                std::mem::swap(&mut buffered_input, &mut state.input);
                std::mem::swap(&mut our_captures, &mut state.captures);
                all_captures = captured_groups
                    .iter()
                    .chain(vec![String::new()].iter())
                    .chain(our_captures.iter())
                    .map(|c| c.clone())
                    .collect();
                backtrack_info = Some(state.backtrack);
                std::mem::swap(&mut matcher_iter, &mut state.matchers_to_resume);
                backtrack_matcher_iter = matcher_iter.clone();
            }

            'match_loop: while let Some(m) = matcher_iter.next() {
                let mut backtrack_state = GroupBacktrackState {
                    input: buffered_input.clone(),
                    captures: our_captures.clone(),
                    backtrack: BacktrackInfo::None,
                    matchers_to_resume: backtrack_matcher_iter.clone(),
                };
                backtrack_matcher_iter.next();

                let (matched, captures, backtrack) =
                    m.test(&mut buffered_input, &all_captures, backtrack_info.take());

                if !matched {
                    if let Some(mut state) = backtrack_stack.pop() {
                        debug!(
                            "Match failed, backtracking: {:?}, {:?}",
                            buffered_input.pop_divided(),
                            state
                        );
                        std::mem::swap(&mut buffered_input, &mut state.input);
                        std::mem::swap(&mut our_captures, &mut state.captures);
                        all_captures = captured_groups
                            .iter()
                            .chain(vec![String::new()].iter())
                            .chain(our_captures.iter())
                            .map(|c| c.clone())
                            .collect();
                        backtrack_info = Some(state.backtrack);
                        std::mem::swap(&mut matcher_iter, &mut state.matchers_to_resume);
                        backtrack_matcher_iter = matcher_iter.clone();
                        continue 'match_loop;
                    } else {
                        trace!("Match failed, nothing to backtrack");
                        continue 'option_loop;
                    }
                } else {
                    for capture in captures {
                        our_captures.push(capture.clone());
                        all_captures.push(capture);
                    }
                    if let Some(backtrack) = backtrack {
                        backtrack_state.backtrack = backtrack;
                        backtrack_stack.push(backtrack_state);
                    }
                }
            }

            let matched_value = buffered_input
                .pop_divided()
                .expect("We have subdivided before, popping should succeed");
            std::mem::swap(input, &mut buffered_input);

            our_captures.insert(0, matched_value);
            return (
                true,
                our_captures,
                Some(BacktrackInfo::Group(option_id, backtrack_stack)),
            );
        }

        (false, Vec::new(), None)
    }
}

#[derive(Debug, Clone)]
pub struct Pattern {
    matchers: Vec<Matcher>,
}

impl Pattern {
    pub fn new(input: &str) -> Result<Self> {
        let mut input = input.chars().peekable();
        let mut matchers = Vec::new();
        while let Some(_) = input.peek() {
            matchers.push(Matcher::new(&mut input)?);
        }

        Ok(Self { matchers })
    }

    pub fn test(&self, input: &str) -> bool {
        self.run(input).0
    }

    pub fn run(&self, input: &str) -> (bool, String, Vec<String>) {
        let mut iter = BufferedIterator::new(input.chars().enumerate());

        while let Some(_) = iter.peek() {
            let mut buffered_iter = iter.clone();
            let (matched, captured) = self.test_section(&mut buffered_iter);
            if matched {
                let (_, buffered) = buffered_iter.take();
                return (true, buffered, captured);
            }
            iter.next();
        }

        (false, String::new(), Vec::new())
    }

    fn test_section<T>(&self, input: &mut BufferedIterator<T>) -> (bool, Vec<String>)
    where
        T: Iterator<Item = (usize, char)> + Clone + Debug,
    {
        let matcher = Matcher::CaptureGroup(self.matchers.clone());
        let (matched, mut captures, _backtrack) = matcher.test(input, &Vec::new(), None);

        if matched {
            captures.remove(0);
            (true, captures)
        } else {
            (false, Vec::new())
        }
    }
}

#[cfg(test)]
mod test {
    use crate::Pattern;

    #[test]
    fn single_character_match() {
        let pattern = Pattern::new("x").expect("Pattern is correct");
        assert!(!pattern.test(""));
        assert!(!pattern.test("X"));
        assert!(!pattern.test("abc"));
        assert!(pattern.test("x"));
        assert!(pattern.test("xylophone"));
        assert!(pattern.test("lax"));
        assert!(pattern.test("taxi"));
    }

    #[test]
    fn literal_match() {
        let pattern = Pattern::new("abc").expect("Pattern is correct");
        assert!(pattern.test("abc"));
        assert!(pattern.test("abcde"));
        assert!(pattern.test("012abcde"));
        assert!(!pattern.test("lax"));
        assert!(!pattern.test("abxc"));
    }

    #[test]
    fn digit_match() {
        let pattern = Pattern::new(r"\d").expect("Pattern is correct");
        assert!(pattern.test("1"));
        assert!(pattern.test("a2"));
        assert!(pattern.test("012abcde"));
        assert!(!pattern.test("lax"));
        assert!(!pattern.test("abxc"));
    }

    #[test]
    fn alphanumeric_match() {
        let pattern = Pattern::new(r"\w").expect("Pattern is correct");
        assert!(pattern.test("1"));
        assert!(pattern.test("a"));
        assert!(pattern.test("Z"));
        assert!(pattern.test("_"));
        assert!(!pattern.test("-"));
        assert!(!pattern.test(":"));
    }

    #[test]
    fn group_match() {
        let pattern = Pattern::new(r"[a\d]").expect("Pattern is correct");
        assert!(pattern.test("1"));
        assert!(pattern.test("a"));
        assert!(pattern.test("9"));
        assert!(pattern.test("za"));
        assert!(!pattern.test("b"));
        assert!(!pattern.test(":"));
    }

    #[test]
    fn negative_group_match() {
        let pattern = Pattern::new(r"[^a\d]").expect("Pattern is correct");
        assert!(!pattern.test("1"));
        assert!(!pattern.test("a"));
        assert!(!pattern.test("9"));
        assert!(pattern.test("za"));
        assert!(pattern.test("b"));
        assert!(pattern.test(":"));
    }

    #[test]
    fn start_of_string_match() {
        let pattern = Pattern::new(r"^a").expect("Pattern is correct");
        assert!(pattern.test("a"));
        assert!(pattern.test("ab"));
        assert!(!pattern.test("ba"));
    }

    #[test]
    fn end_of_string_match() {
        let pattern = Pattern::new(r"a$").expect("Pattern is correct");
        assert!(pattern.test("a"));
        assert!(!pattern.test("ab"));
        assert!(pattern.test("ba"));
    }

    #[test]
    fn one_or_more_match() {
        let pattern = Pattern::new(r"ab+c").expect("Pattern is correct");
        assert!(pattern.test("abc"));
        assert!(pattern.test("abbc"));
        assert!(pattern.test("abbbc"));
        assert!(!pattern.test("ac"));
    }

    #[test]
    fn zero_or_more_match() {
        let pattern = Pattern::new(r"ab*c").expect("Pattern is correct");
        assert!(pattern.test("abc"));
        assert!(pattern.test("abbc"));
        assert!(pattern.test("abbbc"));
        assert!(pattern.test("ac"));
    }

    #[test]
    fn zero_or_one_match() {
        let pattern = Pattern::new(r"ab?c").expect("Pattern is correct");
        assert!(pattern.test("abc"));
        assert!(!pattern.test("abbc"));
        assert!(!pattern.test("abbbc"));
        assert!(pattern.test("ac"));
    }

    #[test]
    fn alternative() {
        let pattern = Pattern::new(r"(abc|xyz)\d").expect("Pattern is correct");
        assert!(pattern.test("abc1"));
        assert!(pattern.test("xyz2"));
        assert!(!pattern.test("xyz"));
    }

    #[test]
    fn match_test() {
        let pattern = Pattern::new(r"([abc]+)(\d+)").expect("Pattern is correct");
        let (matched, all, groups) = pattern.run("abc123");
        assert!(matched);
        assert_eq!(all, "abc123");
        assert_eq!(groups[0], "abc");
        assert_eq!(groups[1], "123");
    }

    #[test]
    fn nested_match_test() {
        let pattern = Pattern::new(r"'((\w+) and cat)'").expect("Pattern is correct");
        let (matched, all, groups) = pattern.run("'dog and cat'");
        assert!(matched);
        assert_eq!(all, "'dog and cat'");
        assert_eq!(groups[0], "dog and cat");
        assert_eq!(groups[1], "dog");
    }

    #[test]
    fn backreference_test() {
        let pattern = Pattern::new(r"(\w+) and \1").expect("Pattern is correct");
        assert!(pattern.test("cat and cat"));
        assert!(!pattern.test("cat and dog2"));
    }

    #[test]
    fn nested_backreference_test() {
        let pattern =
            Pattern::new(r"('(\w+) and \2') is the same as \1").expect("Pattern is correct");
        assert!(pattern.test("'cat and cat' is the same as 'cat and cat'"));
    }

    #[test]
    fn double_nested_backreference_test() {
        let pattern =
            Pattern::new(r"('((\w+) and) \3') is the same as \1").expect("Pattern is correct");
        assert!(pattern.test("'cat and cat' is the same as 'cat and cat'"));
    }

    #[test]
    fn backtracking() {
        let pattern = Pattern::new(r"\w+a").expect("Pattern is correct");
        assert!(pattern.test("mocha"));
    }

    #[test]
    fn backtracking_in_group() {
        let pattern = Pattern::new(r"([^xyz]+)a").expect("Pattern is correct");
        assert!(pattern.test("mocha"));
    }

    #[test]
    fn zero_or_one_backtrack() {
        let pattern = Pattern::new(r"t?t").expect("Pattern is correct");
        assert!(pattern.test("tt"));
        assert!(pattern.test("t"));
    }

    #[test]
    fn backreference_after_backtrack() {
        let pattern = Pattern::new(r"(t)t?\1").expect("Pattern is correct");
        assert!(pattern.test("ttt"));
        assert!(pattern.test("tt"));
    }

    #[test]
    fn last_stage_test() {
        let pattern = Pattern::new(r"(([abc]+)-([def]+)) is \1, not ([^xyz]+), \2, or \3")
            .expect("Pattern is correct");
        //        let pattern =
        //            Pattern::new(r"((abc)-(def)) is \1, not (efg,?), \2").expect("Pattern is correct");
        assert!(pattern.test("abc-def is abc-def, not efg, abc, or def"));
    }

    #[test]
    fn full_test() {
        let pattern = Pattern::new(r"a\d[\w:][^x]").expect("Pattern is correct");
        assert!(pattern.test("a9cv"));
        assert!(pattern.test("da4cg"));
        assert!(!pattern.test("da4cx"));
        assert!(!pattern.test("ab9cv"));
        assert!(!pattern.test("ab9Xv"));
        assert!(!pattern.test("ab9_v"));
        assert!(!pattern.test("ab9:v"));
    }
}
