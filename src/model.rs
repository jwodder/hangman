use crate::words::Word;
use std::collections::BTreeMap;
use thiserror::Error;

/// The 26 uppercase letters of the ASCII alphabet, for use as the `alphabet`
/// argument to [`Hangman::new()`]
pub(crate) static ASCII_ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// The state of the hangman's gallows in a game of Hangman
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Gallows {
    /// The initial state, when no incorrect guesses have yet been made
    Start,
    /// The state when one incorrect guess has been made
    AddHead,
    /// The state when two incorrect guesses have been made
    AddTorso,
    /// The state when three incorrect guesses have been made
    AddLeftArm,
    /// The state when four incorrect guesses have been made
    AddRightArm,
    /// The state when five incorrect guesses have been made
    AddLeftLeg,
    /// The state when six incorrect guesses (the maximum) have been made
    AddRightLeg,
}

impl Gallows {
    /// Alias for the final `Gallows` state
    pub(crate) const END: Gallows = Gallows::AddRightLeg;

    /// Return the next gallows state, if any
    pub(crate) fn succ(self) -> Option<Gallows> {
        match self {
            Gallows::Start => Some(Gallows::AddHead),
            Gallows::AddHead => Some(Gallows::AddTorso),
            Gallows::AddTorso => Some(Gallows::AddLeftArm),
            Gallows::AddLeftArm => Some(Gallows::AddRightArm),
            Gallows::AddRightArm => Some(Gallows::AddLeftLeg),
            Gallows::AddLeftLeg => Some(Gallows::AddRightLeg),
            Gallows::AddRightLeg => None,
        }
    }
}

/// Outcome of a completed game of Hangman
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Fate {
    /// The user won
    Won,
    /// The user lost by making too many incorrect guesses
    Lost(Lost),
}

/// Outcome of a guess in Hangman
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Response {
    /// The guessed character was in the secret word and had not been
    /// previously guessed
    GoodGuess {
        /// The guessed character, converted to uppercase if ASCII
        guess: char,
        /// The number of occurrences of the guess in the secret word
        count: usize,
        /// True iff the user won the game with this guess
        won: bool,
    },
    /// The guessed character was not in the secret word
    BadGuess {
        /// The guessed character, converted to uppercase if ASCII
        guess: char,
        /// `Some` iff the user lost the game with this guess
        lost: Option<Lost>,
    },
    /// The user guessed a character that had already been guessed
    AlreadyGuessed {
        /// The guessed character, converted to uppercase if ASCII
        guess: char,
    },
    /// The user guessed a character that was not in the game's alphabet
    InvalidGuess {
        /// The guessed character, converted to uppercase if ASCII
        guess: char,
    },
    /// [`Hangman::guess()`] was called after the game ended (i.e., when
    /// [`Hangman::fate()`] is returning `Some`)
    GameOver,
}

/// Details on a game that the user lost
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Lost {
    /// The secret word in its entirety, as a consolation prize
    pub(crate) word: Vec<char>,
}

/// A game of Hangman.
///
/// Text provided to a `Hangman` instance — be it the word or alphabet provided
/// on construction or a character supplied as a guess — is normalized by
/// converting lowercase ASCII letters to uppercase.  No other normalization is
/// performed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Hangman {
    /// Mapping from normalized characters in the alphabet to whether they've
    /// been guessed (true) or not (false)
    letters: BTreeMap<char, bool>,
    gallows: Gallows,
    word: Vec<char>,
    /// A representation of the characters in the word known by the user.
    /// `known_letters` is the same length as `word`.  At each index `i`,
    /// `known_letters[i]` is either `Some(word[i])` if the characters therein
    /// is known to the user (either because they guessed it previously or
    /// because the character is not in the game's alphabet and thus was
    /// revealed from the start) and `None` otherwise.
    known_letters: Vec<Option<char>>,
    fate: Option<Fate>,
}

impl Hangman {
    /// Create a game of Hangman in which the secret word is `word` and the
    /// user must guess characters from `alphabet`.
    ///
    /// Characters in `word` and `alphabet` are normalized by converting
    /// lowercase ASCII letters to uppercase.
    ///
    /// `word` need not be limited to the characters in `alphabet`; any
    /// characters in `word` outside of `alphabet` will start out revealed to
    /// the user without having to be guessed.
    pub(crate) fn new(word: Word, alphabet: &str) -> Result<Hangman, HangmanError> {
        let letters: BTreeMap<char, bool> = alphabet
            .chars()
            .map(|c| (normalize_char(c), false))
            .collect();
        let word: Vec<char> = word.as_ref().chars().map(normalize_char).collect();
        let known_letters: Vec<_> = word
            .iter()
            .map(|&c| (!letters.contains_key(&c)).then_some(c))
            .collect();
        if known_letters.iter().all(Option::is_some) {
            return Err(HangmanError::NoAlphabet);
        }
        Ok(Hangman {
            letters,
            gallows: Gallows::Start,
            word,
            known_letters,
            fate: None,
        })
    }

    /// Process a guess at a character in the secret word.
    ///
    /// If `guess` is ASCII, it is handled case-insensitively.
    ///
    /// If the game has ended (i.e., if [`Hangman::fate()`] is returning
    /// `Some`), this method will return [`Response::GameOver`].
    pub(crate) fn guess(&mut self, guess: char) -> Response {
        if self.fate().is_some() {
            return Response::GameOver;
        }
        let guess = normalize_char(guess);
        match self.letters.get_mut(&guess) {
            Some(true) => Response::AlreadyGuessed { guess },
            Some(b @ false) => {
                let mut count = 0;
                for (&wch, known) in self.word.iter().zip(self.known_letters.iter_mut()) {
                    if wch == guess {
                        debug_assert!(
                            known.is_none(),
                            "Newly-guessed letter should not have already been revealed"
                        );
                        count += 1;
                        *known = Some(wch);
                    }
                }
                *b = true;
                if count > 0 {
                    let won = if self.known_letters.iter().all(Option::is_some) {
                        self.fate = Some(Fate::Won);
                        true
                    } else {
                        false
                    };
                    Response::GoodGuess { guess, count, won }
                } else {
                    if let Some(g) = self.gallows.succ() {
                        self.gallows = g;
                    }
                    let lost = (self.gallows == Gallows::END).then(|| {
                        let about = Lost {
                            word: self.word.clone(),
                        };
                        self.fate = Some(Fate::Lost(about.clone()));
                        about
                    });
                    Response::BadGuess { guess, lost }
                }
            }
            None => Response::InvalidGuess { guess },
        }
    }

    /// Returns a mapping from characters in the game's alphabet (with
    /// lowercase ASCII letters converted to uppercase) to either `true` (if
    /// the character has been guessed by the user) or `false` (if the user
    /// hasn't guessed it yet)
    pub(crate) fn guessed(&self) -> &BTreeMap<char, bool> {
        &self.letters
    }

    /// Returns the current state of the hangman's gallows
    pub(crate) fn gallows(&self) -> Gallows {
        self.gallows
    }

    /// Returns the secret word as revealed to the user so far, with lowercase
    /// ASCII letters converted to uppercase.  Each element of the slice is
    /// either `Some(ch)` (if `ch` was previously guessed successfully by the
    /// user or if `ch` is a character in the secret word that does not appear
    /// in the alphabet and thus was revealed from the start) or `None` (if the
    /// user has yet to guess the underlying character).
    pub(crate) fn known_letters(&self) -> &[Option<char>] {
        &self.known_letters
    }

    /// If the game has ended, returns `Some(fate)`, where `fate` describes the
    /// outcome.  Otherwise, returns `None`.
    pub(crate) fn fate(&self) -> Option<Fate> {
        self.fate.clone()
    }
}

#[derive(Copy, Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum HangmanError {
    #[error("secret word must contain at least one letter from the alphabet")]
    NoAlphabet,
}

fn normalize_char(c: char) -> char {
    c.to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gallows_end() {
        let iter = std::iter::successors(Some(Gallows::Start), |&g| g.succ());
        assert_eq!(Gallows::END, iter.last().unwrap());
    }
}
