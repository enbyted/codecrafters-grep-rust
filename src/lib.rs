use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unexpected end of input")]
    EOF,
    #[error("Unknown character group `\\{0}`")]
    UnknownCharacterType(char),
}

pub type Result<T> = std::result::Result<T, Error>;

enum Matcher {
    Literal(char),
    Digit,
}

impl Matcher {
    pub fn new(input: &mut impl Iterator<Item = char>) -> Result<Self> {
        match input.next() {
            Some('\\') => match input.next() {
                Some('\\') => Ok(Self::Literal('\\')),
                Some('d') => Ok(Self::Digit),
                Some(ch) => Err(Error::UnknownCharacterType(ch)),
                None => Err(Error::EOF),
            },
            Some(ch) => Ok(Self::Literal(ch)),
            None => Err(Error::EOF),
        }
    }

    pub fn test(&self, input: &mut impl Iterator<Item = char>) -> bool {
        match self {
            Matcher::Literal(c) => {
                if let Some(ch) = input.next() {
                    ch == *c
                } else {
                    false
                }
            }
            Matcher::Digit => {
                if let Some(ch) = input.next() {
                    ch.is_ascii_digit()
                } else {
                    false
                }
            }
        }
    }
}

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
        let mut iter = input.chars().peekable();

        while let Some(_) = iter.peek() {
            if self.test_section(iter.clone()) {
                return true;
            }
            iter.next();
        }

        false
    }

    fn test_section(&self, mut input: impl Iterator<Item = char>) -> bool {
        for matcher in &self.matchers {
            if !matcher.test(&mut input) {
                return false;
            }
        }

        true
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
    fn full_test() {
        let pattern = Pattern::new(r"a\dc").expect("Pattern is correct");
        assert!(pattern.test("a9c"));
        assert!(pattern.test("da4cg"));
        assert!(!pattern.test("ab9c"));
    }
}
