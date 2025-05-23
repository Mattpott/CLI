use std::borrow::Cow;

use textwrap::{
    core::{Fragment, Word},
    wrap_algorithms,
};
use unicode_width::UnicodeWidthChar;

/// Code for computing the number of displayed characters which the previous
/// char (Unicode scalar value) took up
/// Code adapted/gotten from gobang
pub fn compute_character_width(c: char) -> u16 {
    UnicodeWidthChar::width(c).unwrap_or(0) as u16
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct WhiteSpaceWord<'a> {
    /// Word content, which may be a single whitespace character
    pub(crate) word: &'a str,
    // Cached width in columns.
    pub(crate) width: u16,
    /// Whitespace to insert if the word does not fall at the end of a line.
    pub(crate) whitespace: &'a str,
    /// Penalty string to insert if the word falls at the end of a line.
    pub(crate) penalty: &'a str,
}

impl WhiteSpaceWord<'_> {
    /// Returns true if the stored "word" is a single newline character ('\n')
    fn is_newline(&self) -> bool {
        self.word.len() == 1 && self.word.chars().nth(0) == Some('\n')
    }
}

impl<'a> WhiteSpaceWord<'a> {
    fn with_whitespace(self, whitespace: &'a str) -> Self {
        Self {
            word: self.word,
            width: self.width,
            whitespace,
            penalty: self.penalty,
        }
    }

    /// Break this word into smaller words with a width of at most
    /// `line_width`. The whitespace and penalty from this `Word` is
    /// added to the last piece.
    ///
    /// Code adapted from textwrap's Word struct:
    /// https://github.com/mgeisler/textwrap/blob/c9bd8b0b807b1b62e388e5aeb9a3d7f3276cff84/src/core.rs#L286
    fn break_apart<'b>(&'b self, line_width: u16) -> impl Iterator<Item = WhiteSpaceWord<'a>> + 'b {
        let mut char_indices = self.word.char_indices();
        let mut offset = 0;
        let mut width = 0;

        std::iter::from_fn(move || {
            for (idx, ch) in char_indices.by_ref() {
                if width > 0 && width + compute_character_width(ch) > line_width {
                    let word = WhiteSpaceWord {
                        word: &self.word[offset..idx],
                        width,
                        whitespace: "",
                        penalty: "",
                    };
                    offset = idx;
                    width = compute_character_width(ch);
                    return Some(word);
                }

                width += compute_character_width(ch);
            }

            if offset < self.word.len() {
                let word = WhiteSpaceWord {
                    word: &self.word[offset..],
                    width,
                    whitespace: self.whitespace,
                    penalty: self.penalty,
                };
                offset = self.word.len();
                return Some(word);
            }

            None
        })
    }
}

impl Fragment for WhiteSpaceWord<'_> {
    fn width(&self) -> f64 {
        self.width as f64
    }

    fn whitespace_width(&self) -> f64 {
        self.whitespace.len() as f64
    }

    fn penalty_width(&self) -> f64 {
        self.penalty.len() as f64
    }
}

// Used to turn WhiteSpaceWord into a "smart pointer" which can be immutably
// dereferenced using the * operator, or implicitly dereferenced in some cases.
// This is useful for treating the word as just a single layer of abstraction
// over the word stored within it
impl std::ops::Deref for WhiteSpaceWord<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.word
    }
}

impl<'a> From<&'a str> for WhiteSpaceWord<'a> {
    fn from(word: &'a str) -> Self {
        // as all WhiteSpaceWords are broken into fragments that may include
        // single whitespace characters, there is no need to trim the text
        let width = textwrap::core::display_width(word) as u16;
        WhiteSpaceWord {
            word,
            width,
            whitespace: "",
            penalty: "",
        }
    }
}

impl<'a> From<&WhiteSpaceWord<'a>> for Word<'a> {
    fn from(value: &WhiteSpaceWord<'a>) -> Self {
        Word::from(value.word)
    }
}

impl<'a> From<WhiteSpaceWord<'a>> for Word<'a> {
    fn from(value: WhiteSpaceWord<'a>) -> Self {
        Word::from(value.word)
    }
}

impl<'a> From<&Word<'a>> for WhiteSpaceWord<'a> {
    fn from(value: &Word<'a>) -> Self {
        WhiteSpaceWord {
            word: value.word,
            width: value.width() as u16, // this could potentially lose size, but whatever
            whitespace: value.whitespace,
            penalty: value.penalty,
        }
    }
}

impl<'a> From<Word<'a>> for WhiteSpaceWord<'a> {
    fn from(value: Word<'a>) -> Self {
        WhiteSpaceWord::from(&value)
    }
}

fn separate_into_fragments(text: &str) -> impl Iterator<Item = WhiteSpaceWord> {
    // iterate over each character and determine the
    // slice for each word within the passed text
    let mut start = 0;
    let mut prev_char = '\0';
    let mut char_indices = text.char_indices();
    std::iter::from_fn(move || {
        for (i, c) in char_indices.by_ref() {
            // if previous fragment was a word that captured a single
            // trailing whitespace, continue to next character
            if start >= i && i != 0 {
                prev_char = c;
                continue;
            }
            // capture prev_char into its own fragment as it is whitespace
            if prev_char.is_whitespace() {
                let word = WhiteSpaceWord::from(&text[start..i]);
                prev_char = c;
                start = i;
                return Some(word);
            } else if c.is_whitespace() {
                // words can have 1 trailing whitespace character that doesn't
                // wrap to the next line, so capture c as whitespace unless
                // it is a newline character, which is its own fragment
                let word = WhiteSpaceWord::from(&text[start..i]);
                // TODO: POTENTIALLY FIX THIS?
                // if c != '\n' {
                //     let end = i + c.len_utf8();
                //     word = word.with_whitespace(&text[i..end]);
                //     start = end; // skip over c
                // } else {
                //     start = i;
                // };
                start = i;
                prev_char = c;
                return Some(word);
            }
            // still within a word, so keep parsing
            prev_char = c;
        }
        // capture any remaining characters in the last fragment
        if start < text.len() {
            let word = WhiteSpaceWord::from(&text[start..]);
            start = text.len();
            return Some(word);
        }
        // end the iterator
        None
    })
}

/// Forcibly break words wider than `line_width` into smaller words.
///
/// Code adapted from textwrap's core.rs function of the same name:
/// https://github.com/mgeisler/textwrap/blob/c9bd8b0b807b1b62e388e5aeb9a3d7f3276cff84/src/core.rs#L354
fn break_words<'a, I>(words: I, line_width: u16) -> Vec<WhiteSpaceWord<'a>>
where
    I: IntoIterator<Item = WhiteSpaceWord<'a>>,
{
    let mut shortened_words = Vec::new();
    for word in words {
        if word.width > line_width {
            shortened_words.extend(word.break_apart(line_width));
        } else {
            shortened_words.push(word);
        }
    }
    shortened_words
}

/// Wrap a line of text at a given width.
///
/// Code adapted from textwrap's wrap.rs function of the same name:
/// https://github.com/mgeisler/textwrap/blob/master/src/wrap.rs#L180
pub fn wrap(text: &str, width: u16) -> Vec<Cow<'_, str>> {
    let mut lines = Vec::new();
    // split only on linefeed characters, but keep them in the string
    // as it is important for calculation of display length
    for line in text.split_inclusive('\n') {
        wrap_single_line(line, width, &mut lines);
    }
    lines
}

/// Wrap a line of text at a given width.
///
/// Code adapted from textwrap's wrap.rs function of the same name:
/// https://github.com/mgeisler/textwrap/blob/master/src/wrap.rs#L195
fn wrap_single_line<'a>(line: &'a str, width: u16, lines: &mut Vec<Cow<'a, str>>) {
    // if the length of the line is already less than width, we are good
    if line.len() < width.into() {
        lines.push(Cow::from(line));
    } else {
        wrap_single_line_slow_path(line, width, lines)
    }
}

/// Wrap a single line of text.
///
/// This is taken when `line` is longer than `options.width`.
///
/// Code adapted from textwrap's wrap.rs function of the same name:
/// https://github.com/mgeisler/textwrap/blob/master/src/wrap.rs#L215
fn wrap_single_line_slow_path<'a>(line: &'a str, width: u16, lines: &mut Vec<Cow<'a, str>>) {
    let words = separate_into_fragments(line);
    let broken_words = break_words(words, width);
    let wrapped_words = wrap_algorithms::wrap_first_fit(broken_words.as_slice(), &[width as f64]);

    let mut idx = 0;
    for words in wrapped_words {
        let last_word = match words.last() {
            None => {
                lines.push(Cow::from(""));
                continue;
            }
            Some(word) => word,
        };

        // We assume here that all words are contiguous in `line`.
        // That is, the sum of their lengths should add up to the
        // length of `line`.
        let len = words
            .iter()
            .map(|word| word.len() + word.whitespace.len())
            .sum::<usize>()
            - last_word.whitespace.len();

        // borrow the resulting slice
        let mut result = Cow::from(&line[idx..idx + len]);
        // add any penalty, which is unused for me
        if !last_word.penalty.is_empty() {
            result.to_mut().push_str(last_word.penalty);
        }
        lines.push(result);

        // Advance by the length of `result`, plus the length of
        // `last_word.whitespace` -- even if we had a penalty, we need
        // to skip over the whitespace.
        idx += len + last_word.whitespace.len();
    }
}
