use crate::model::Gallows;
use console::{measure_text_width, truncate_str};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{read, KeyCode, KeyEvent, KeyModifiers},
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
    frame: Frame,
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
            frame: content.render(),
        })
    }

    pub(crate) fn read_guess(&mut self) -> Result<Option<char>, ScreenError> {
        let normal_modifiers = KeyModifiers::NONE | KeyModifiers::SHIFT;
        loop {
            let event = read().map_err(ScreenError::Read)?;
            if let Some(KeyEvent {
                code, modifiers, ..
            }) = event.as_key_press_event()
            {
                if code == KeyCode::Esc
                    || (modifiers, code) == (KeyModifiers::CONTROL, KeyCode::Char('c'))
                {
                    return Ok(None);
                }
                if normal_modifiers.contains(modifiers) {
                    if let KeyCode::Char(ch) = code {
                        return Ok(Some(ch));
                    }
                }
                self.beep()?;
            } else if let Some((columns, rows)) = event.as_resize_event() {
                self.columns = columns;
                self.rows = rows;
                self.draw()?;
            }
        }
    }

    pub(crate) fn pause(&mut self) -> Result<(), ScreenError> {
        self.read_guess().map(|_| ())
    }

    pub(crate) fn update(&mut self, content: Content) -> Result<(), ScreenError> {
        self.frame = content.render();
        self.draw()?;
        Ok(())
    }

    pub(crate) fn draw(&mut self) -> Result<(), ScreenError> {
        queue!(self.inner, Clear(ClearType::All)).map_err(ScreenError::Write)?;
        for (y, x, ln) in self.frame.lines_in_area(self.columns, self.rows) {
            queue!(self.inner, MoveTo(x, y), Print(ln)).map_err(ScreenError::Write)?;
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
struct Frame {
    lines: Vec<String>,
    width: usize,
    height: usize,
}

impl Frame {
    fn with_capacity(capacity: usize) -> Frame {
        Frame {
            lines: Vec::with_capacity(capacity),
            width: 0,
            height: 0,
        }
    }

    fn push<S: Into<String>>(&mut self, line: S) {
        let line = line.into();
        self.width = self.width.max(measure_text_width(&line));
        self.height += 1;
        self.lines.push(line);
    }

    fn pad_to_width(&mut self, width: usize) {
        self.width = self.width.max(width);
    }

    fn push_centered<S: Into<String>>(&mut self, line: S) {
        let mut line = line.into();
        let width = measure_text_width(&line);
        if width > self.width {
            let indent = " ".repeat((width - self.width) / 2);
            for ln in &mut self.lines {
                if !ln.is_empty() {
                    ln.insert_str(0, &indent);
                }
            }
            self.width = width;
        } else if width < self.width && !line.is_empty() {
            line = " ".repeat((self.width - width) / 2) + &line;
        }
        self.height += 1;
        self.lines.push(line);
    }

    fn lines_in_area(
        &self,
        width: u16,
        height: u16,
    ) -> impl Iterator<Item = (u16, u16, String)> + '_ {
        let width = usize::from(width);
        let Ok(left_margin) = u16::try_from(width.saturating_sub(self.width) / 2) else {
            unreachable!("(u16 - int) / 2 should fit in a u16");
        };
        let height = usize::from(height);
        let Ok(top_margin) = u16::try_from(height.saturating_sub(self.height) / 2) else {
            unreachable!("(u16 - int) / 2 should fit in a u16");
        };
        self.lines
            .iter()
            .take(height)
            .zip(top_margin..)
            .map(move |(line, y)| (y, left_margin, truncate_str(line, width, "").into_owned()))
    }
}

impl Extend<String> for Frame {
    fn extend<I: IntoIterator<Item = String>>(&mut self, iter: I) {
        for line in iter {
            self.push(line);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Content {
    pub(crate) hint: Option<String>,
    pub(crate) gallows: Gallows,
    pub(crate) guess_options: Vec<Option<char>>,
    pub(crate) word_display: Vec<CharDisplay>,
    pub(crate) message: Message,
}

impl Content {
    const GALLOWS_HEIGHT: usize = 5;
    const GALLOWS_WIDTH: usize = 8;
    const LETTER_COLUMNS: usize = 6;
    const GUTTER: usize = 4;
    const WIDTH: usize =
        Content::GALLOWS_WIDTH + Content::GUTTER + (Content::LETTER_COLUMNS * 2) - 1;
    const HEIGHT: usize = Content::GALLOWS_HEIGHT + 8;

    fn render(self) -> Frame {
        let mut frame = Frame::with_capacity(Content::HEIGHT);
        if let Some(hint) = self.hint {
            frame.push(format!("Hint: {hint}"));
        } else {
            frame.push(String::new());
        }
        frame.push(String::new());
        let mut hud = Vec::with_capacity(Content::GALLOWS_HEIGHT);
        for row in Content::draw_gallows(self.gallows, self.message.gallows_advanced()) {
            hud.push(format!("{}{:gutter$}", row, "", gutter = Content::GUTTER));
        }
        for (i, optchunk) in self
            .guess_options
            .chunks(Content::LETTER_COLUMNS)
            .enumerate()
        {
            let ln = if let Some(ln) = hud.get_mut(i) {
                ln
            } else {
                hud.push(" ".repeat(Content::GALLOWS_WIDTH + Content::GUTTER));
                hud.last_mut()
                    .expect("lines should not be empty after pushing")
            };
            let mut first = true;
            for opt in optchunk {
                if !std::mem::replace(&mut first, false) {
                    ln.push(' ');
                }
                ln.push(opt.unwrap_or(' '));
            }
        }
        frame.extend(hud);
        frame.push(String::new());
        frame.pad_to_width(Content::WIDTH);
        let mut wordline = String::with_capacity(self.word_display.len() * 2 - 1);
        let mut first = true;
        for ch in self.word_display {
            if !std::mem::replace(&mut first, false) {
                wordline.push(' ');
            }
            write!(wordline, "{ch}").unwrap();
        }
        frame.push_centered(wordline);
        frame.push(String::new());
        frame.push_centered(self.message.to_string());
        frame.push(String::new());
        if self.message.is_game_over() {
            frame.push_centered("Press the Any Key to exit.");
        } else {
            frame.push(String::new());
        }
        frame
    }

    #[rustfmt::skip]
    fn draw_gallows(
        gallows: Gallows,
        highlight: bool,
    ) -> &'static [&'static str; Content::GALLOWS_HEIGHT] {
        match (gallows, highlight) {
            (Gallows::Start, _) => &[
                "  ┌───┐ ",
                "  │     ",
                "  │     ",
                "  │     ",
                "──┴──   "
            ],
            (Gallows::AddHead, false) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │     ",
                "  │     ",
                "──┴──   "
            ],
            (Gallows::AddHead, true) => &[
                "  ┌───┐ ",
                "  │   \x1B[1;31mo\x1B[m ",
                "  │     ",
                "  │     ",
                "──┴──   ",
            ],
            (Gallows::AddTorso, false) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │   | ",
                "  │     ",
                "──┴──   ",
            ],
            (Gallows::AddTorso, true) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │   \x1B[1;31m|\x1B[m ",
                "  │     ",
                "──┴──   ",
            ],
            (Gallows::AddLeftArm, false) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │  /| ",
                "  │     ",
                "──┴──   ",
            ],
            (Gallows::AddLeftArm, true) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │  \x1B[1;31m/\x1B[m| ",
                "  │     ",
                "──┴──   ",
            ],
            (Gallows::AddRightArm, false) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │  /|\\",
                "  │     ",
                "──┴──   ",
            ],
            (Gallows::AddRightArm, true) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │  /|\x1B[1;31m\\\x1B[m",
                "  │     ",
                "──┴──   ",
            ],
            (Gallows::AddLeftLeg, false) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │  /|\\",
                "  │  /  ",
                "──┴──   ",
            ],
            (Gallows::AddLeftLeg, true) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │  /|\\",
                "  │  \x1B[1;31m/\x1B[m  ",
                "──┴──   ",
            ],
            (Gallows::AddRightLeg, false) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │  /|\\",
                "  │  / \\",
                "──┴──   ",
            ],
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
pub(crate) enum CharDisplay {
    Plain(char),
    Highlighted(char),
    Blank,
}

impl fmt::Display for CharDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CharDisplay::Plain(ch) => write!(f, "{ch}"),
            CharDisplay::Highlighted(ch) => write!(f, "\x1B[1m{ch}\x1B[m"),
            CharDisplay::Blank => write!(f, "_"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Message {
    Start,
    GoodGuess { guess: char, count: usize },
    BadGuess { guess: char },
    AlreadyGuessed { guess: char },
    InvalidGuess { guess: char },
    Won,
    Lost,
}

impl Message {
    fn is_game_over(&self) -> bool {
        matches!(self, Message::Won | Message::Lost)
    }

    fn gallows_advanced(&self) -> bool {
        matches!(self, Message::BadGuess { .. } | Message::Lost)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

    #[test]
    fn test_gallows_widths() {
        for gallows in std::iter::successors(Some(Gallows::Start), |&g| g.succ()) {
            for line in Content::draw_gallows(gallows, false) {
                assert_eq!(measure_text_width(line), Content::GALLOWS_WIDTH);
            }
            for line in Content::draw_gallows(gallows, true) {
                assert_eq!(measure_text_width(line), Content::GALLOWS_WIDTH);
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
            };
            let frame = content.render();
            assert_eq!(
                frame.lines,
                [
                    "   Hint: A difficult word",
                    "",
                    "     ┌───┐     A B C D E F",
                    "     │         G H I J K L",
                    "     │         M N O P Q R",
                    "     │         S T U V W X",
                    "   ──┴──       Y Z",
                    "",
                    "         _ _ _ _ _ _",
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
            };
            let frame = content.render();
            assert_eq!(
                frame.lines,
                [
                    "",
                    "",
                    "     ┌───┐     A B C D E F",
                    "     │         G H I J K L",
                    "     │         M N O P Q R",
                    "     │         S T U V W X",
                    "   ──┴──       Y Z",
                    "",
                    "         _ _ _ _ _ _",
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
            };
            let frame = content.render();
            assert_eq!(
                frame.lines,
                [
                    "        Hint: A difficult word",
                    "",
                    "          ┌───┐       B C D E F",
                    "          │         G H I J K L",
                    "          │         M N O P Q R",
                    "          │         S T U V W X",
                    "        ──┴──       Y Z",
                    "",
                    "              \x1B[1mA\x1B[m _ \x1B[1mA\x1B[m _ _ _",
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
            };
            let frame = content.render();
            assert_eq!(
                frame.lines,
                [
                    "      Hint: A difficult word",
                    "",
                    "        ┌───┐       B C D   F",
                    "        │   \x1B[1;31mo\x1B[m     G H I J K L",
                    "        │         M N O P Q R",
                    "        │         S T U V W X",
                    "      ──┴──       Y Z",
                    "",
                    "            A _ A _ _ _",
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
            };
            let frame = content.render();
            assert_eq!(
                frame.lines,
                [
                    " Hint: A difficult word",
                    "",
                    "   ┌───┐               F",
                    "   │   o     G H   J K L",
                    "   │  /|\\    M N O P Q R",
                    "   │               V W X",
                    " ──┴──       Y Z",
                    "",
                    "       A B A C U S",
                    "",
                    "        You win!",
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
            };
            let frame = content.render();
            assert_eq!(
                frame.lines,
                [
                    " Hint: A difficult word",
                    "",
                    "   ┌───┐       B C D   F",
                    "   │   o     G H   J K L",
                    "   │  /|\\    M N   P Q  ",
                    "   │  / \x1B[1;31m\\\x1B[m    S     V W X",
                    " ──┴──         Z",
                    "",
                    "       A \x1B[1mB\x1B[m A \x1B[1mC\x1B[m U \x1B[1mS\x1B[m",
                    "",
                    " Oh dear, you are dead!",
                    "",
                    "Press the Any Key to exit.",
                ]
            );
        }
    }
}
