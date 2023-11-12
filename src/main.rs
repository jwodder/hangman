mod model;
mod view;
use crate::model::*;
use crate::view::*;
use rand::seq::IteratorRandom;
use std::io;

static WORDS: &str = include_str!("words.txt");

fn main() -> io::Result<()> {
    let word = WORDS
        .lines()
        .choose(&mut rand::thread_rng())
        .expect("wordlist should be nonempty");
    let mut game = Hangman::new(word, ASCII_ALPHABET);
    let content = Content {
        gallows: game.gallows(),
        guess_options: game.guess_options(),
        known_letters: game.known_letters().to_vec(),
        message: Message::Start,
    };
    let mut screen = Screen::new(io::stdout(), content)?;
    screen.draw()?;
    loop {
        let guess = screen.getchar()?;
        let r = game.guess(guess);
        let message = if let Some(fate) = game.fate() {
            match fate {
                Fate::Won => Message::Won,
                Fate::Lost => Message::Lost,
                Fate::OutOfLetters => Message::OutOfLetters,
            }
        } else {
            match r {
                Response::GoodGuess { letters_revealed } => Message::GoodGuess {
                    guess,
                    letters_revealed,
                },
                Response::BadGuess => Message::BadGuess {
                    guess,
                    mistakes_left: game.mistakes_left(),
                },
                Response::AlreadyGuessed => Message::AlreadyGuessed { guess },
                Response::InvalidGuess => Message::InvalidGuess { guess },
                Response::GameOver => Message::InvalidGuess { guess },
            }
        };
        let content = Content {
            gallows: game.gallows(),
            guess_options: game.guess_options(),
            known_letters: game.known_letters().to_vec(),
            message,
        };
        screen.update(content)?;
        if game.fate().is_some() {
            let _ = screen.getchar();
            break;
        }
    }
    Ok(())
}
