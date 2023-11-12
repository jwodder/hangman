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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Screen<W: Write> {
    inner: W,
    columns: u16,
    rows: u16,
    lines: Vec<String>,
}

impl<W: Write> Screen<W> {
    pub(crate) fn new(mut inner: W, content: Content) -> io::Result<Screen<W>> {
        let (columns, rows) = size()?;
        inner.execute(EnterAlternateScreen)?;
        if let Err(e) = enable_raw_mode() {
            let _ = inner.execute(LeaveAlternateScreen);
            return Err(e);
        }
        if let Err(e) = inner.execute(Hide) {
            let _ = disable_raw_mode();
            let _ = inner.execute(LeaveAlternateScreen);
            return Err(e);
        }
        Ok(Screen {
            inner,
            columns,
            rows,
            lines: content.render(),
        })
    }

    pub(crate) fn getchar(&mut self) -> io::Result<char> {
        fn extract_char(code: KeyCode, modifiers: KeyModifiers) -> Option<char> {
            let normal_modifiers = KeyModifiers::NONE | KeyModifiers::SHIFT;
            if normal_modifiers.contains(modifiers) {
                if let KeyCode::Char(c) = code {
                    return Some(c);
                }
            }
            None
        }

        loop {
            match read()? {
                Event::Key(KeyEvent {
                    code,
                    modifiers,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    if let Some(ch) = extract_char(code, modifiers) {
                        return Ok(ch);
                    } else {
                        self.beep()?;
                    }
                }
                Event::Resize(columns, rows) => {
                    // TODO: Debounce resize floods
                    self.columns = columns;
                    self.rows = rows;
                    self.draw()?;
                }
                _ => (),
            }
        }
    }

    pub(crate) fn update(&mut self, content: Content) -> io::Result<()> {
        self.lines = content.render();
        self.draw()?;
        Ok(())
    }

    pub(crate) fn draw(&mut self) -> io::Result<()> {
        let left_margin = match u16::try_from(Content::WIDTH) {
            Ok(width) => self.columns.saturating_sub(width) / 2,
            Err(_) => 0,
        };
        let top_margin = match u16::try_from(self.lines.len()) {
            Ok(length) => self.rows.saturating_sub(length) / 2,
            Err(_) => 0,
        };
        queue!(self.inner, Clear(ClearType::All))?;
        for (y, ln) in std::iter::zip(top_margin.., &self.lines) {
            queue!(self.inner, MoveTo(left_margin, y), Print(ln))?;
        }
        self.inner.flush()?;
        Ok(())
    }

    fn beep(&mut self) -> io::Result<()> {
        self.inner.execute(Print("\x07"))?;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Content {
    pub(crate) gallows: Gallows,
    pub(crate) guess_options: Vec<Option<char>>,
    pub(crate) known_letters: Vec<Option<char>>,
    pub(crate) message: Message,
}

impl Content {
    const GALLOWS_HEIGHT: usize = 5;
    const GALLOWS_WIDTH: usize = 8;
    const LETTER_COLUMNS: usize = 6;
    const GUTTER: usize = 4;
    const WIDTH: usize =
        Content::GALLOWS_WIDTH + Content::GUTTER + (Content::LETTER_COLUMNS * 2) - 1;

    fn render(self) -> Vec<String> {
        let mut lines = Vec::with_capacity(Content::GALLOWS_HEIGHT + 4);
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
            let ln = match lines.get_mut(i) {
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

        let indent = Content::WIDTH.saturating_sub(self.known_letters.len()) / 2;
        let mut wordline = " ".repeat(indent);
        let highlight = if let Message::GoodGuess { guess, .. } = self.message {
            Some(guess)
        } else {
            None
        };
        let mut first = true;
        for opt in self.known_letters {
            if !std::mem::replace(&mut first, false) {
                wordline.push(' ');
            }
            match opt {
                Some(ch) if opt == highlight => write!(wordline, "\x1B[1m{}\x1B[m", ch).unwrap(),
                Some(ch) => wordline.push(ch),
                None => wordline.push('_'),
            }
        }
        lines.push(wordline);
        lines.push(String::new());
        lines.push(self.message.to_string());
        lines
    }

    fn draw_gallows(
        gallows: Gallows,
        highlight: bool,
    ) -> &'static [&'static str; Content::GALLOWS_HEIGHT] {
        match (gallows, highlight) {
            (Gallows::Empty, _) => &["  ┌───┐ ", "  │     ", "  │     ", "  │     ", "──┴──   "],
            (Gallows::AddHead, false) => {
                &["  ┌───┐ ", "  │   o ", "  │     ", "  │     ", "──┴──   "]
            }
            (Gallows::AddHead, true) => &[
                "  ┌───┐ ",
                "  │   \x1B[31mo\x1B[m ",
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
                "  │   \x1B[31m|\x1B[m ",
                "  │     ",
                "──┴──   ",
            ],
            (Gallows::AddLeftArm, false) => {
                &["  ┌───┐ ", "  │   o ", "  │  /| ", "  │     ", "──┴──   "]
            }
            (Gallows::AddLeftArm, true) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │  \x1B[31m/\x1B[m| ",
                "  │     ",
                "──┴──   ",
            ],
            (Gallows::AddRightArm, false) => {
                &["  ┌───┐ ", "  │   o ", "  │  /|\\", "  │     ", "──┴──   "]
            }
            (Gallows::AddRightArm, true) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │  /|\x1B[31m\\\x1B[m",
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
                "  │  \x1B[31m/\x1B[m  ",
                "──┴──   ",
            ],
            (Gallows::AddRightLeg, false) => {
                &["  ┌───┐ ", "  │   o ", "  │  /|\\", "  │  / \\", "──┴──   "]
            }
            (Gallows::AddRightLeg, true) => &[
                "  ┌───┐ ",
                "  │   o ",
                "  │  /|\\",
                "  │  / \x1B[31m\\\x1B[m",
                "──┴──   ",
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Message {
    Start,
    GoodGuess {
        guess: char,
        letters_revealed: usize,
    },
    BadGuess {
        guess: char,
        mistakes_left: usize,
    },
    AlreadyGuessed {
        guess: char,
    },
    InvalidGuess {
        guess: char,
    },
    Won,
    Lost,
    OutOfLetters,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Message::Start => write!(f, "Try to guess the secret word!"),
            Message::GoodGuess {
                guess,
                letters_revealed,
            } => {
                write!(f, "Correct!  There ")?;
                if *letters_revealed == 1 {
                    write!(f, "is 1 {guess:?} ")?;
                } else {
                    write!(f, "are {letters_revealed} {guess:?}s ")?;
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
            Message::OutOfLetters => write!(f, "You've exhausted the alphabet!"),
        }
    }
}
