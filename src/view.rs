use crate::model::Gallows;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{read, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    queue,
    style::Print,
    terminal::{
        disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
    ExecutableCommand,
};
use std::fmt::{self, Write as _};
use std::io::{self, Write};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Screen<W: Write> {
    inner: W,
    columns: u16,
    rows: u16,
    lines: Vec<String>,
}

impl<W: Write> Screen<W> {
    pub(crate) fn new(mut inner: W, content: Content) -> Result<Screen<W>, ScreenError> {
        let (columns, rows) = size().map_err(ScreenError::Init)?;
        inner
            .execute(EnterAlternateScreen)
            .map_err(ScreenError::Init)?;
        if let Err(e) = enable_raw_mode() {
            let _ = inner.execute(LeaveAlternateScreen);
            return Err(ScreenError::Init(e));
        }
        if let Err(e) = inner.execute(Hide) {
            let _ = disable_raw_mode();
            let _ = inner.execute(LeaveAlternateScreen);
            return Err(ScreenError::Init(e));
        }
        Ok(Screen {
            inner,
            columns,
            rows,
            lines: content.render(),
        })
    }

    pub(crate) fn read_guess(&mut self) -> Result<Option<char>, ScreenError> {
        let normal_modifiers = KeyModifiers::NONE | KeyModifiers::SHIFT;
        loop {
            match read().map_err(ScreenError::Read)? {
                Event::Key(KeyEvent {
                    code: KeyCode::Esc, ..
                }) => return Ok(None),
                Event::Key(KeyEvent {
                    code,
                    modifiers,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    if normal_modifiers.contains(modifiers) {
                        if let KeyCode::Char(ch) = code {
                            return Ok(Some(ch));
                        }
                    }
                    self.beep()?;
                }
                Event::Resize(columns, rows) => {
                    self.columns = columns;
                    self.rows = rows;
                    self.draw()?;
                }
                _ => (),
            }
        }
    }

    pub(crate) fn pause(&mut self) -> Result<(), ScreenError> {
        self.read_guess().map(|_| ())
    }

    pub(crate) fn update(&mut self, content: Content) -> Result<(), ScreenError> {
        self.lines = content.render();
        self.draw()?;
        Ok(())
    }

    pub(crate) fn draw(&mut self) -> Result<(), ScreenError> {
        let left_margin = match u16::try_from(Content::WIDTH) {
            Ok(width) => self.columns.saturating_sub(width) / 2,
            Err(_) => 0,
        };
        let top_margin = match u16::try_from(self.lines.len()) {
            Ok(length) => self.rows.saturating_sub(length) / 2,
            Err(_) => 0,
        };
        queue!(self.inner, Clear(ClearType::All)).map_err(ScreenError::Write)?;
        for (y, ln) in std::iter::zip(top_margin.., &self.lines) {
            queue!(self.inner, MoveTo(left_margin, y), Print(ln)).map_err(ScreenError::Write)?;
        }
        self.inner.flush().map_err(ScreenError::Write)?;
        Ok(())
    }

    fn beep(&mut self) -> Result<(), ScreenError> {
        self.inner
            .execute(Print("\x07"))
            .map_err(ScreenError::Write)?;
        Ok(())
    }
}

impl<W: Write> Drop for Screen<W> {
    fn drop(&mut self) {
        let _ = self.inner.execute(Show);
        let _ = disable_raw_mode();
        let _ = self.inner.execute(LeaveAlternateScreen);
    }
}

#[derive(Debug, Error)]
pub(crate) enum ScreenError {
    #[error("failed to initialize terminal display")]
    Init(#[source] io::Error),
    #[error("failed to read from terminal")]
    Read(#[source] io::Error),
    #[error("failed to write to terminal")]
    Write(#[source] io::Error),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Content {
    pub(crate) hint: Option<String>,
    pub(crate) gallows: Gallows,
    pub(crate) guess_options: Vec<Option<char>>,
    pub(crate) word_display: Vec<CharDisplay>,
    pub(crate) message: Message,
    pub(crate) game_over: bool,
}

impl Content {
    const GALLOWS_HEIGHT: usize = 5;
    const GALLOWS_WIDTH: usize = 8;
    const LETTER_COLUMNS: usize = 6;
    const GUTTER: usize = 4;
    const WIDTH: usize =
        Content::GALLOWS_WIDTH + Content::GUTTER + (Content::LETTER_COLUMNS * 2) - 1;

    fn render(self) -> Vec<String> {
        let mut lines = Vec::with_capacity(Content::GALLOWS_HEIGHT + 8);
        if let Some(hint) = self.hint {
            lines.push(format!("Hint: {hint}"));
        } else {
            lines.push(String::new());
        }
        lines.push(String::new());
        for row in Content::draw_gallows(
            self.gallows,
            matches!(self.message, Message::BadGuess { .. } | Message::Lost),
        ) {
            lines.push(format!("{}{:gutter$}", row, "", gutter = Content::GUTTER));
        }
        for (i, optchunk) in self
            .guess_options
            .chunks(Content::LETTER_COLUMNS)
            .enumerate()
        {
            let ln = match lines.get_mut(i + 2) {
                Some(ln) => ln,
                None => {
                    lines.push(" ".repeat(Content::GALLOWS_WIDTH + Content::GUTTER));
                    lines.last_mut().unwrap()
                }
            };
            let mut first = true;
            for opt in optchunk {
                if !std::mem::replace(&mut first, false) {
                    ln.push(' ');
                }
                ln.push(opt.unwrap_or(' '));
            }
        }
        lines.push(String::new());
        let indent = Content::WIDTH.saturating_sub(self.word_display.len() * 2 - 1) / 2;
        let mut wordline = " ".repeat(indent);
        let mut first = true;
        for ch in self.word_display {
            if !std::mem::replace(&mut first, false) {
                wordline.push(' ');
            }
            write!(wordline, "{ch}").unwrap();
        }
        lines.push(wordline);
        lines.push(String::new());
        lines.push(self.message.to_string());
        lines.push(String::new());
        if self.game_over {
            lines.push(String::from("Press the Any Key to exit."));
        } else {
            lines.push(String::new());
        }
        lines
    }

    fn draw_gallows(
        gallows: Gallows,
        highlight: bool,
    ) -> &'static [&'static str; Content::GALLOWS_HEIGHT] {
        match (gallows, highlight) {
            (Gallows::Start, _) => &["  ┌───┐ ", "  │     ", "  │     ", "  │     ", "──┴──   "],
            (Gallows::AddHead, false) => {
                &["  ┌───┐ ", "  │   o ", "  │     ", "  │     ", "──┴──   "]
            }
            (Gallows::AddHead, true) => &[
                "  ┌───┐ ",
                "  │   \x1B[1;31mo\x1B[m ",
                "  │     ",
                "  │     ",
                "──┴──   ",
            ],
            (Gallows::AddTorso, false) => {
                &["  ┌───┐ ", "  │   o ", "  │   | ", "  │     ", "──┴──   "]
            }
            (Gallows::AddTorso, true) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │   \x1B[1;31m|\x1B[m ",
                "  │     ",
                "──┴──   ",
            ],
            (Gallows::AddLeftArm, false) => {
                &["  ┌───┐ ", "  │   o ", "  │  /| ", "  │     ", "──┴──   "]
            }
            (Gallows::AddLeftArm, true) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │  \x1B[1;31m/\x1B[m| ",
                "  │     ",
                "──┴──   ",
            ],
            (Gallows::AddRightArm, false) => {
                &["  ┌───┐ ", "  │   o ", "  │  /|\\", "  │     ", "──┴──   "]
            }
            (Gallows::AddRightArm, true) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │  /|\x1B[1;31m\\\x1B[m",
                "  │     ",
                "──┴──   ",
            ],
            (Gallows::AddLeftLeg, false) => {
                &["  ┌───┐ ", "  │   o ", "  │  /|\\", "  │  /  ", "──┴──   "]
            }
            (Gallows::AddLeftLeg, true) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │  /|\\",
                "  │  \x1B[1;31m/\x1B[m  ",
                "──┴──   ",
            ],
            (Gallows::AddRightLeg, false) => {
                &["  ┌───┐ ", "  │   o ", "  │  /|\\", "  │  / \\", "──┴──   "]
            }
            (Gallows::AddRightLeg, true) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │  /|\\",
                "  │  / \x1B[1;31m\\\x1B[m",
                "──┴──   ",
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CharDisplay {
    Plain(char),
    Highlighted(char),
    Blank,
}

impl fmt::Display for CharDisplay {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CharDisplay::Plain(ch) => write!(f, "{ch}"),
            CharDisplay::Highlighted(ch) => write!(f, "\x1B[1m{ch}\x1B[m"),
            CharDisplay::Blank => write!(f, "_"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Message {
    Start,
    GoodGuess { guess: char, count: usize },
    BadGuess { guess: char },
    AlreadyGuessed { guess: char },
    InvalidGuess { guess: char },
    Won,
    Lost,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Message::Start => write!(f, "Try to guess the secret word!"),
            Message::GoodGuess { guess, count } => {
                write!(f, "Correct!  There ")?;
                if *count == 1 {
                    write!(f, "is 1 {guess:?} ")?;
                } else {
                    write!(f, "are {count} {guess:?}s ")?;
                }
                write!(f, "in the word.")?;
                Ok(())
            }
            Message::BadGuess { guess, .. } => {
                write!(f, "Wrong!  There's no {guess:?} in the word.")
            }
            Message::AlreadyGuessed { guess } => {
                write!(f, "You already guessed {guess:?}.")
            }
            Message::InvalidGuess { guess } => {
                write!(f, "{guess:?} is not an option.")
            }
            Message::Won => write!(f, "You win!"),
            Message::Lost => write!(f, "Oh dear, you are dead!"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn test_gallows_widths() {
        for gallows in std::iter::successors(Some(Gallows::Start), |&g| g.succ()) {
            for line in Content::draw_gallows(gallows, false) {
                assert_eq!(UnicodeWidthStr::width(*line), Content::GALLOWS_WIDTH);
            }
            for line in Content::draw_gallows(gallows, false) {
                let line = strip_ansi_escapes::strip_str(line);
                assert_eq!(UnicodeWidthStr::width(&*line), Content::GALLOWS_WIDTH);
            }
        }
    }

    mod content_render {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn start() {
            let content = Content {
                hint: Some(String::from("A difficult word")),
                gallows: Gallows::Start,
                guess_options: vec![
                    Some('A'),
                    Some('B'),
                    Some('C'),
                    Some('D'),
                    Some('E'),
                    Some('F'),
                    Some('G'),
                    Some('H'),
                    Some('I'),
                    Some('J'),
                    Some('K'),
                    Some('L'),
                    Some('M'),
                    Some('N'),
                    Some('O'),
                    Some('P'),
                    Some('Q'),
                    Some('R'),
                    Some('S'),
                    Some('T'),
                    Some('U'),
                    Some('V'),
                    Some('W'),
                    Some('X'),
                    Some('Y'),
                    Some('Z'),
                ],
                word_display: vec![
                    CharDisplay::Blank,
                    CharDisplay::Blank,
                    CharDisplay::Blank,
                    CharDisplay::Blank,
                    CharDisplay::Blank,
                    CharDisplay::Blank,
                ],
                message: Message::Start,
                game_over: false,
            };
            let lines = content.render();
            assert_eq!(
                lines,
                [
                    "Hint: A difficult word",
                    "",
                    "  ┌───┐     A B C D E F",
                    "  │         G H I J K L",
                    "  │         M N O P Q R",
                    "  │         S T U V W X",
                    "──┴──       Y Z",
                    "",
                    "      _ _ _ _ _ _",
                    "",
                    "Try to guess the secret word!",
                    "",
                    "",
                ]
            );
        }

        #[test]
        fn no_hint() {
            let content = Content {
                hint: None,
                gallows: Gallows::Start,
                guess_options: vec![
                    Some('A'),
                    Some('B'),
                    Some('C'),
                    Some('D'),
                    Some('E'),
                    Some('F'),
                    Some('G'),
                    Some('H'),
                    Some('I'),
                    Some('J'),
                    Some('K'),
                    Some('L'),
                    Some('M'),
                    Some('N'),
                    Some('O'),
                    Some('P'),
                    Some('Q'),
                    Some('R'),
                    Some('S'),
                    Some('T'),
                    Some('U'),
                    Some('V'),
                    Some('W'),
                    Some('X'),
                    Some('Y'),
                    Some('Z'),
                ],
                word_display: vec![
                    CharDisplay::Blank,
                    CharDisplay::Blank,
                    CharDisplay::Blank,
                    CharDisplay::Blank,
                    CharDisplay::Blank,
                    CharDisplay::Blank,
                ],
                message: Message::Start,
                game_over: false,
            };
            let lines = content.render();
            assert_eq!(
                lines,
                [
                    "",
                    "",
                    "  ┌───┐     A B C D E F",
                    "  │         G H I J K L",
                    "  │         M N O P Q R",
                    "  │         S T U V W X",
                    "──┴──       Y Z",
                    "",
                    "      _ _ _ _ _ _",
                    "",
                    "Try to guess the secret word!",
                    "",
                    "",
                ]
            );
        }

        #[test]
        fn after_good_guess() {
            let content = Content {
                hint: Some(String::from("A difficult word")),
                gallows: Gallows::Start,
                guess_options: vec![
                    None,
                    Some('B'),
                    Some('C'),
                    Some('D'),
                    Some('E'),
                    Some('F'),
                    Some('G'),
                    Some('H'),
                    Some('I'),
                    Some('J'),
                    Some('K'),
                    Some('L'),
                    Some('M'),
                    Some('N'),
                    Some('O'),
                    Some('P'),
                    Some('Q'),
                    Some('R'),
                    Some('S'),
                    Some('T'),
                    Some('U'),
                    Some('V'),
                    Some('W'),
                    Some('X'),
                    Some('Y'),
                    Some('Z'),
                ],
                word_display: vec![
                    CharDisplay::Highlighted('A'),
                    CharDisplay::Blank,
                    CharDisplay::Highlighted('A'),
                    CharDisplay::Blank,
                    CharDisplay::Blank,
                    CharDisplay::Blank,
                ],
                message: Message::GoodGuess {
                    guess: 'A',
                    count: 2,
                },
                game_over: false,
            };
            let lines = content.render();
            assert_eq!(
                lines,
                [
                    "Hint: A difficult word",
                    "",
                    "  ┌───┐       B C D E F",
                    "  │         G H I J K L",
                    "  │         M N O P Q R",
                    "  │         S T U V W X",
                    "──┴──       Y Z",
                    "",
                    "      \x1B[1mA\x1B[m _ \x1B[1mA\x1B[m _ _ _",
                    "",
                    "Correct!  There are 2 'A's in the word.",
                    "",
                    "",
                ]
            );
        }

        #[test]
        fn after_bad_guess() {
            let content = Content {
                hint: Some(String::from("A difficult word")),
                gallows: Gallows::AddHead,
                guess_options: vec![
                    None,
                    Some('B'),
                    Some('C'),
                    Some('D'),
                    None,
                    Some('F'),
                    Some('G'),
                    Some('H'),
                    Some('I'),
                    Some('J'),
                    Some('K'),
                    Some('L'),
                    Some('M'),
                    Some('N'),
                    Some('O'),
                    Some('P'),
                    Some('Q'),
                    Some('R'),
                    Some('S'),
                    Some('T'),
                    Some('U'),
                    Some('V'),
                    Some('W'),
                    Some('X'),
                    Some('Y'),
                    Some('Z'),
                ],
                word_display: vec![
                    CharDisplay::Plain('A'),
                    CharDisplay::Blank,
                    CharDisplay::Plain('A'),
                    CharDisplay::Blank,
                    CharDisplay::Blank,
                    CharDisplay::Blank,
                ],
                message: Message::BadGuess { guess: 'E' },
                game_over: false,
            };
            let lines = content.render();
            assert_eq!(
                lines,
                [
                    "Hint: A difficult word",
                    "",
                    "  ┌───┐       B C D   F",
                    "  │   \x1B[1;31mo\x1B[m     G H I J K L",
                    "  │         M N O P Q R",
                    "  │         S T U V W X",
                    "──┴──       Y Z",
                    "",
                    "      A _ A _ _ _",
                    "",
                    "Wrong!  There's no 'E' in the word.",
                    "",
                    "",
                ]
            );
        }

        #[test]
        fn win() {
            let content = Content {
                hint: Some(String::from("A difficult word")),
                gallows: Gallows::AddRightArm,
                guess_options: vec![
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some('F'),
                    Some('G'),
                    Some('H'),
                    None,
                    Some('J'),
                    Some('K'),
                    Some('L'),
                    Some('M'),
                    Some('N'),
                    Some('O'),
                    Some('P'),
                    Some('Q'),
                    Some('R'),
                    None,
                    None,
                    None,
                    Some('V'),
                    Some('W'),
                    Some('X'),
                    Some('Y'),
                    Some('Z'),
                ],
                word_display: vec![
                    CharDisplay::Plain('A'),
                    CharDisplay::Plain('B'),
                    CharDisplay::Plain('A'),
                    CharDisplay::Plain('C'),
                    CharDisplay::Plain('U'),
                    CharDisplay::Plain('S'),
                ],
                message: Message::Won,
                game_over: true,
            };
            let lines = content.render();
            assert_eq!(
                lines,
                [
                    "Hint: A difficult word",
                    "",
                    "  ┌───┐               F",
                    "  │   o     G H   J K L",
                    "  │  /|\\    M N O P Q R",
                    "  │               V W X",
                    "──┴──       Y Z",
                    "",
                    "      A B A C U S",
                    "",
                    "You win!",
                    "",
                    "Press the Any Key to exit.",
                ]
            );
        }

        #[test]
        fn lose() {
            let content = Content {
                hint: Some(String::from("A difficult word")),
                gallows: Gallows::AddRightLeg,
                guess_options: vec![
                    None,
                    Some('B'),
                    Some('C'),
                    Some('D'),
                    None,
                    Some('F'),
                    Some('G'),
                    Some('H'),
                    None,
                    Some('J'),
                    Some('K'),
                    Some('L'),
                    Some('M'),
                    Some('N'),
                    None,
                    Some('P'),
                    Some('Q'),
                    None,
                    Some('S'),
                    None,
                    None,
                    Some('V'),
                    Some('W'),
                    Some('X'),
                    None,
                    Some('Z'),
                ],
                word_display: vec![
                    CharDisplay::Plain('A'),
                    CharDisplay::Highlighted('B'),
                    CharDisplay::Plain('A'),
                    CharDisplay::Highlighted('C'),
                    CharDisplay::Plain('U'),
                    CharDisplay::Highlighted('S'),
                ],
                message: Message::Lost,
                game_over: true,
            };
            let lines = content.render();
            assert_eq!(
                lines,
                [
                    "Hint: A difficult word",
                    "",
                    "  ┌───┐       B C D   F",
                    "  │   o     G H   J K L",
                    "  │  /|\\    M N   P Q  ",
                    "  │  / \x1B[1;31m\\\x1B[m    S     V W X",
                    "──┴──         Z",
                    "",
                    "      A \x1B[1mB\x1B[m A \x1B[1mC\x1B[m U \x1B[1mS\x1B[m",
                    "",
                    "Oh dear, you are dead!",
                    "",
                    "Press the Any Key to exit.",
                ]
            );
        }
    }
}
