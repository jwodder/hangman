use std::collections::BTreeMap;
use strum::{EnumCount, EnumIter, IntoEnumIterator};

pub(crate) static ASCII_ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";

#[derive(Clone, Copy, Debug, EnumCount, EnumIter, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Gallows {
    Empty,
    AddHead,
    AddTorso,
    AddLeftArm,
    AddRightArm,
    AddLeftLeg,
    AddRightLeg,
}

impl Gallows {
    pub(crate) const END: Gallows = Gallows::AddRightLeg;
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Fate {
    Won,
    Lost,
    OutOfLetters,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Response {
    GoodGuess { letters_revealed: usize },
    BadGuess,
    AlreadyGuessed,
    // Guess was not in the allowed alphabet
    InvalidGuess,
    // User guessed after game was over
    GameOver,
}

#[derive(Clone, Debug)]
pub(crate) struct Hangman {
    // Mapping of allowed guesses to whether they've been guessed (true) or not
    // (false)
    letters: BTreeMap<char, bool>,
    gallows: Gallows,
    gallows_iter: GallowsIter,
    word: Vec<char>,
    known_letters: Vec<Option<char>>,
    fate: Option<Fate>,
}

impl Hangman {
    pub(crate) fn new(word: &str, alphabet: &str) -> Hangman {
        let letters: BTreeMap<char, bool> = alphabet
            .chars()
            .map(|c| (normalize_char(c), false))
            .collect();
        let word: Vec<char> = word.chars().map(normalize_char).collect();
        let known_letters = word
            .iter()
            .map(|&c| (!letters.contains_key(&c)).then_some(c))
            .collect();
        let mut gallows_iter = Gallows::iter();
        let gallows = gallows_iter.next().expect("GallowsIter should be nonempty");
        Hangman {
            letters,
            gallows,
            gallows_iter,
            word,
            known_letters,
            fate: None,
        }
    }

    pub(crate) fn guess(&mut self, ch: char) -> Response {
        if self.fate().is_some() {
            return Response::GameOver;
        }
        let ch = normalize_char(ch);
        match self.letters.get_mut(&ch) {
            Some(true) => Response::AlreadyGuessed,
            Some(b @ false) => {
                let mut letters_revealed = 0;
                for (&wch, known) in self.word.iter().zip(self.known_letters.iter_mut()) {
                    if wch == ch {
                        debug_assert!(known.is_none());
                        letters_revealed += 1;
                        *known = Some(wch);
                    }
                }
                *b = true;
                let r = if letters_revealed > 0 {
                    Response::GoodGuess { letters_revealed }
                } else {
                    if let Some(g) = self.gallows_iter.next() {
                        self.gallows = g;
                    }
                    Response::BadGuess
                };
                self.determine_fate();
                r
            }
            None => Response::InvalidGuess,
        }
    }

    pub(crate) fn guess_options(&self) -> Vec<Option<char>> {
        self.letters
            .iter()
            .map(|(&ch, &b)| (!b).then_some(ch))
            .collect()
    }

    /*
    pub(crate) fn mistakes_made(&self) -> usize {
        self.gallows as usize
    }

    pub(crate) fn max_mistakes() -> usize {
        Gallows::COUNT
    }

    pub(crate) fn mistakes_left(&self) -> usize {
        Hangman::max_mistakes() - self.mistakes_made()
    }
    */

    pub(crate) fn gallows(&self) -> Gallows {
        self.gallows
    }

    pub(crate) fn known_letters(&self) -> &[Option<char>] {
        &self.known_letters
    }

    pub(crate) fn word(&self) -> &[char] {
        &self.word
    }

    fn determine_fate(&mut self) {
        self.fate = if self.known_letters.iter().all(Option::is_some) {
            Some(Fate::Won)
        } else if self.gallows == Gallows::END {
            Some(Fate::Lost)
        } else if self.letters.values().all(|&b| b) {
            Some(Fate::OutOfLetters)
        } else {
            None
        }
    }

    pub(crate) fn fate(&self) -> Option<Fate> {
        self.fate
    }
}

pub(crate) fn normalize_char(c: char) -> char {
    c.to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gallows_end() {
        assert_eq!(Gallows::END, Gallows::iter().last().unwrap());
    }
}
