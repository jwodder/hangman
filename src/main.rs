mod model;
mod view;
mod words;
use crate::model::*;
use crate::view::*;
use crate::words::*;
use lexopt::{Arg, Parser, ValueExt};
use patharg::InputArg;
use std::io;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Command {
    Run(WordSource),
    Help,
    Version,
}

impl Command {
    fn from_parser(mut parser: Parser) -> Result<Command, lexopt::Error> {
        let mut word_source = WordSource::default();
        while let Some(arg) = parser.next()? {
            match arg {
                Arg::Short('h') | Arg::Long("help") => return Ok(Command::Help),
                Arg::Short('V') | Arg::Long("version") => return Ok(Command::Version),
                Arg::Short('w') | Arg::Long("word") => {
                    word_source = WordSource::Fixed(parser.value()?.parse()?)
                }
                Arg::Short('f') | Arg::Long("words-file") => {
                    word_source = WordSource::File(InputArg::from_arg(parser.value()?))
                }
                _ => return Err(arg.unexpected()),
            }
        }
        Ok(Command::Run(word_source))
    }

    fn run(self) -> anyhow::Result<()> {
        match self {
            Command::Run(word_source) => {
                let WordWithHint { word, hint } = word_source.fetch()?;
                let mut game = Hangman::new(word.as_ref(), ASCII_ALPHABET);
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
                while let Some(guess) = screen.read_guess()? {
                    let r = game.guess(guess);
                    let mut word_display = display_known_letters(game.known_letters());
                    let mut game_over = false;
                    let mut message = match r {
                        Response::GoodGuess {
                            guess,
                            letters_revealed,
                        } => {
                            for cd in &mut word_display {
                                if *cd == CharDisplay::Plain(guess) {
                                    *cd = CharDisplay::Highlighted(guess);
                                }
                            }
                            Message::GoodGuess {
                                guess,
                                letters_revealed,
                            }
                        }
                        Response::BadGuess { guess } => Message::BadGuess { guess },
                        Response::AlreadyGuessed { guess } => Message::AlreadyGuessed { guess },
                        Response::InvalidGuess { guess } => Message::InvalidGuess { guess },
                        // This can't happen the way we're using the game,
                        // but we should at least do something reasonable.
                        Response::GameOver => Message::InvalidGuess { guess },
                    };
                    if let Some(fate) = game.fate() {
                        game_over = true;
                        message = match fate {
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
                    }
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
                        screen.pause()?;
                        break;
                    }
                }
            }
            Command::Help => {
                println!("Usage: hangman [-f <FILE>|-w <WORD>]");
                println!();
                println!("Play Hangman in your terminal");
                println!();
                println!("Options:");
                println!("  -f <FILE>, --words-file <FILE>");
                println!("                    Select a word at random from <FILE>");
                println!();
                println!("  -w <WORD>, --word <WORD>");
                println!(
                    "                    Use <WORD> as the secret word.  Good for testing and"
                );
                println!("                    playing against others.");
                println!();
                println!("  -h, --help        Display this help message and exit");
                println!("  -V, --version     Show the program version and exit");
            }
            Command::Version => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            }
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    Command::from_parser(Parser::from_env())?.run()
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
