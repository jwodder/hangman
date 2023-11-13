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
            guess_options: self
                .game
                .guessed()
                .iter()
                .map(|(&ch, &b)| (!b).then_some(ch))
                .collect(),
            word_display: display_known_letters(self.game.known_letters()),
            message: Message::Start,
            game_over: false,
        };
        let mut screen = Screen::new(io::stdout(), content)?;
        screen.draw()?;
        while let Some(guess) = screen.read_guess()? {
            let r = self.game.guess(guess);
            let mut word_display = display_known_letters(self.game.known_letters());
            let mut game_over = false;
            let mut message = match r {
                Response::GoodGuess { guess, count } => {
                    for cd in &mut word_display {
                        if *cd == CharDisplay::Plain(guess) {
                            *cd = CharDisplay::Highlighted(guess);
                        }
                    }
                    Message::GoodGuess { guess, count }
                }
                Response::BadGuess { guess } => Message::BadGuess { guess },
                Response::AlreadyGuessed { guess } => Message::AlreadyGuessed { guess },
                Response::InvalidGuess { guess } => Message::InvalidGuess { guess },
                // This can't happen the way we're using the game, but we
                // should at least do something reasonable.
                Response::GameOver => Message::InvalidGuess { guess },
            };
            if let Some(fate) = self.game.fate() {
                game_over = true;
                message = match fate {
                    Fate::Won => Message::Won,
                    Fate::Lost(word) => {
                        for (ch, cd) in std::iter::zip(word, &mut word_display) {
                            if *cd == CharDisplay::Blank {
                                *cd = CharDisplay::Highlighted(ch);
                            }
                        }
                        Message::Lost
                    }
                }
            }
            let content = Content {
                hint: self.hint.clone(),
                gallows: self.game.gallows(),
                guess_options: self
                    .game
                    .guessed()
                    .iter()
                    .map(|(&ch, &b)| (!b).then_some(ch))
                    .collect(),
                word_display,
                message,
                game_over,
            };
            screen.update(content)?;
            if game_over {
                screen.pause()?;
                break;
            }
        }
        Ok(())
    }
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
