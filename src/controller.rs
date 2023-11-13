use crate::model::*;
use crate::view::*;
use crate::words::WordWithHint;
use std::io;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Controller {
    game: Hangman,
    hint: Option<String>,
}

impl Controller {
    pub(crate) fn new(secret: WordWithHint) -> Controller {
        let WordWithHint { word, hint } = secret;
        let game = Hangman::new(word, ASCII_ALPHABET);
        Controller { game, hint }
    }

    pub(crate) fn run(mut self) -> anyhow::Result<()> {
        let content = Content {
            hint: self.hint.clone(),
            gallows: self.game.gallows(),
            guess_options: self.guess_options(),
            word_display: self.word_display(),
            message: Message::Start,
        };
        let mut screen = Screen::new(io::stdout(), content)?;
        screen.draw()?;
        while let Some(guess) = screen.read_guess()? {
            let r = self.game.guess(guess);
            let mut word_display = self.word_display();
            let message = match r {
                Response::GoodGuess { guess, count, won } => {
                    for cd in &mut word_display {
                        if *cd == CharDisplay::Plain(guess) {
                            *cd = CharDisplay::Highlighted(guess);
                        }
                    }
                    if won {
                        Message::Won
                    } else {
                        Message::GoodGuess { guess, count }
                    }
                }
                Response::BadGuess {
                    lost: Some(Lost { word }),
                    ..
                } => {
                    for (ch, cd) in std::iter::zip(word, &mut word_display) {
                        if *cd == CharDisplay::Blank {
                            *cd = CharDisplay::Highlighted(ch);
                        }
                    }
                    Message::Lost
                }
                Response::BadGuess { guess, lost: None } => Message::BadGuess { guess },
                Response::AlreadyGuessed { guess } => Message::AlreadyGuessed { guess },
                Response::InvalidGuess { guess } => Message::InvalidGuess { guess },
                // This can't happen the way we're using the game, but we
                // should at least do something reasonable.
                Response::GameOver => Message::InvalidGuess { guess },
            };
            let content = Content {
                hint: self.hint.clone(),
                gallows: self.game.gallows(),
                guess_options: self.guess_options(),
                word_display,
                message,
            };
            screen.update(content)?;
            if self.game.fate().is_some() {
                screen.pause()?;
                break;
            }
        }
        Ok(())
    }

    fn guess_options(&self) -> Vec<Option<char>> {
        self.game
            .guessed()
            .iter()
            .map(|(&ch, &b)| (!b).then_some(ch))
            .collect()
    }

    fn word_display(&self) -> Vec<CharDisplay> {
        self.game
            .known_letters()
            .iter()
            .map(|&opt| match opt {
                Some(ch) => CharDisplay::Plain(ch),
                None => CharDisplay::Blank,
            })
            .collect()
    }
}
