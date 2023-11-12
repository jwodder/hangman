use anyhow::Context;
use patharg::InputArg;
use rand::seq::IteratorRandom;
use serde::Deserialize;

static WORDS: &[u8] = include_bytes!("words.csv");

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct Word {
    pub(crate) word: String,
    #[serde(default)]
    pub(crate) hint: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum WordSource {
    #[default]
    Builtin,
    Fixed(String),
    File(InputArg),
}

impl WordSource {
    pub(crate) fn fetch(self) -> anyhow::Result<Word> {
        match self {
            WordSource::Builtin => Ok(word_from_csv(WORDS)
                .expect("builtin wordlist should be nonempty")
                .expect("reading builtin wordlist should not fail")),
            WordSource::Fixed(word) => Ok(Word { word, hint: None }),
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

fn word_from_csv<R: std::io::Read>(reader: R) -> Option<Result<Word, csv::Error>> {
    csv::ReaderBuilder::new()
        .flexible(true)
        .has_headers(false)
        .trim(csv::Trim::All)
        .from_reader(reader)
        .into_deserialize::<Word>()
        .choose(&mut rand::thread_rng())
}
