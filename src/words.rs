use anyhow::Context;
use patharg::InputArg;
use rand::seq::IteratorRandom;
use serde::{
    de::{Deserializer, Unexpected, Visitor},
    Deserialize,
};
use std::fmt;
use thiserror::Error;

static WORDS: &[u8] = include_bytes!("words.csv");

#[derive(Clone, Eq, Debug, Hash, PartialEq)]
pub(crate) struct Word(String);

impl AsRef<str> for Word {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl std::str::FromStr for Word {
    type Err = ParseWordError;

    fn from_str(s: &str) -> Result<Word, ParseWordError> {
        let s = s.trim();
        if s.is_empty() {
            Err(ParseWordError)
        } else {
            Ok(Word(s.to_owned()))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("hangman words cannot be empty or all-whitespace")]
pub(crate) struct ParseWordError;

impl<'de> Deserialize<'de> for Word {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct WordVisitor;

        impl Visitor<'_> for WordVisitor {
            type Value = Word;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a string that is neither empty nor all-whitespace")
            }

            fn visit_str<E>(self, input: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                input
                    .parse::<Word>()
                    .map_err(|_| E::invalid_value(Unexpected::Str(input), &self))
            }
        }

        deserializer.deserialize_str(WordVisitor)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct WordWithHint {
    pub(crate) word: Word,
    #[serde(default)]
    pub(crate) hint: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum WordSource {
    #[default]
    Builtin,
    Fixed(Word),
    File(InputArg),
}

impl WordSource {
    pub(crate) fn fetch(self) -> anyhow::Result<WordWithHint> {
        match self {
            WordSource::Builtin => Ok(word_from_csv(WORDS)
                .expect("builtin wordlist should be nonempty")
                .expect("reading builtin wordlist should not fail")),
            WordSource::Fixed(word) => Ok(WordWithHint { word, hint: None }),
            WordSource::File(infile) => {
                let reader = infile.open().context("failed to open words file")?;
                match word_from_csv(reader) {
                    Some(r) => r.context("failed to read words file"),
                    None => anyhow::bail!("No words found in words file"),
                }
            }
        }
    }
}

fn iter_words<R: std::io::Read>(reader: R) -> csv::DeserializeRecordsIntoIter<R, WordWithHint> {
    csv::ReaderBuilder::new()
        .flexible(true)
        .has_headers(false)
        .trim(csv::Trim::All)
        .from_reader(reader)
        .into_deserialize::<WordWithHint>()
}

fn word_from_csv<R: std::io::Read>(reader: R) -> Option<Result<WordWithHint, csv::Error>> {
    iter_words(reader).choose(&mut rand::thread_rng())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nonempty_builtin_list() {
        let builtins = iter_words(WORDS);
        assert!(builtins.count() > 0);
    }

    #[test]
    fn test_builtin_list_ok() {
        let mut builtins = iter_words(WORDS);
        assert!(builtins.all(|r| r.is_ok()));
    }
}
