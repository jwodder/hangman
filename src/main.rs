mod controller;
mod model;
mod view;
mod words;
use crate::controller::Controller;
use crate::words::*;
use lexopt::{Arg, Parser, ValueExt};
use patharg::InputArg;

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
                    word_source = WordSource::Fixed(WordWithHint {
                        word: parser.value()?.parse()?,
                        hint: None,
                    });
                }
                Arg::Short('H') | Arg::Long("hint") => {
                    if let WordSource::Fixed(ref mut wwh) = word_source {
                        wwh.hint = Some(parser.value()?.string()?);
                    } else {
                        return Err("--hint requires --word".into());
                    }
                }
                Arg::Short('f') | Arg::Long("words-file") => {
                    word_source = WordSource::File(InputArg::from_arg(parser.value()?));
                }
                _ => return Err(arg.unexpected()),
            }
        }
        Ok(Command::Run(word_source))
    }

    fn run(self) -> anyhow::Result<()> {
        match self {
            Command::Run(word_source) => Controller::new(word_source.fetch()?)?.run()?,
            Command::Help => {
                println!("Usage: hangman [<options>]");
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
                println!("  -H <HINT>, --hint <HINT>");
                println!("                    Use <HINT> as the hint for a --word.");
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
