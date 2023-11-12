mod model;
mod view;
use crate::model::*;
use crate::view::*;
use rand::seq::IteratorRandom;
use serde::Deserialize;
use std::io;

static WORDS: &[u8] = include_bytes!("words.csv");

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct Word {
    word: String,
    #[serde(default)]
    hint: Option<String>,
}

fn main() -> io::Result<()> {
    let reader = csv::ReaderBuilder::new()
        .flexible(true)
        .has_headers(false)
        .trim(csv::Trim::All)
        .from_reader(WORDS);
    let Word { word, hint } = reader
        .into_deserialize::<Word>()
        .choose(&mut rand::thread_rng())
        .expect("wordlist should be nonempty")
        .expect("reading wordlist should not fail");
    let mut game = Hangman::new(&word, ASCII_ALPHABET);
    let content = Content {
        hint: hint.clone(),
        gallows: game.gallows(),
        guess_options: game.guess_options(),
        word_display: display_known_letters(game.known_letters()),
        message: Message::Start,
        game_over: false,
    };
    let mut screen = Screen::new(io::stdout(), content)?;
    screen.draw()?;
    while let Some(guess) = screen.getchar()? {
        let r = game.guess(guess);
        let mut word_display = display_known_letters(game.known_letters());
        let mut game_over = false;
        let message = if let Some(fate) = game.fate() {
            game_over = true;
            match fate {
                Fate::Won => Message::Won,
                Fate::Lost => {
                    for (&ch, cd) in std::iter::zip(game.word(), &mut word_display) {
                        if *cd == CharDisplay::Blank {
                            *cd = CharDisplay::Highlighted(ch);
                        }
                    }
                    Message::Lost
                }
                Fate::OutOfLetters => Message::OutOfLetters,
            }
        } else {
            match r {
                Response::GoodGuess { letters_revealed } => {
                    let normguess = normalize_char(guess);
                    for cd in &mut word_display {
                        if *cd == CharDisplay::Plain(normguess) {
                            *cd = CharDisplay::Highlighted(normguess);
                        }
                    }
                    Message::GoodGuess {
                        guess,
                        letters_revealed,
                    }
                }
                Response::BadGuess => Message::BadGuess { guess },
                Response::AlreadyGuessed => Message::AlreadyGuessed { guess },
                Response::InvalidGuess => Message::InvalidGuess { guess },
                Response::GameOver => Message::InvalidGuess { guess },
            }
        };
        let content = Content {
            hint: hint.clone(),
            gallows: game.gallows(),
            guess_options: game.guess_options(),
            word_display,
            message,
            game_over,
        };
        screen.update(content)?;
        if game_over {
            let _ = screen.getchar()?;
            break;
        }
    }
    Ok(())
}

fn display_known_letters(known: &[Option<char>]) -> Vec<CharDisplay> {
    known
        .iter()
        .map(|&opt| match opt {
            Some(ch) => CharDisplay::Plain(ch),
            None => CharDisplay::Blank,
        })
        .collect()
}
