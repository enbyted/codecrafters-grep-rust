use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {}

pub type Result<T> = std::result::Result<T, Error>;

enum Matcher {
    Literal(char),
}

impl Matcher {
    pub fn test(&self, input: &mut impl Iterator<Item = char>) -> bool {
        match self {
            Matcher::Literal(c) => {
                if let Some(ch) = input.next() {
                    ch == *c
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
        let matchers = input.chars().map(|c| Matcher::Literal(c)).collect();

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
}
