use crate::{Error, Result};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style, ThemeSet},
    parsing::SyntaxSet,
    util::{LinesWithEndings, as_24_bit_terminal_escaped},
};

pub struct Highlighting<'a> {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    syntax: Option<&'a str>,
    theme: Option<&'a str>,
}

impl<'a> Highlighting<'a> {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            syntax: None,
            theme: None,
        }
    }

    pub fn syntax(mut self, ext: Option<&'a str>) -> Self {
        self.syntax = ext;
        self
    }

    pub fn theme(mut self, theme: Option<&'a str>) -> Self {
        self.theme = theme;
        self
    }

    pub fn highlight(self, text: &str) -> Result<String> {
        let Self {
            syntax_set,
            theme_set,
            syntax,
            theme,
        } = self;

        let syntax = syntax_set
            .find_syntax_by_extension(syntax.unwrap_or("md"))
            .ok_or_else(|| Error::NotFound(String::from("could not find extension")))?;

        let theme = theme_set
            .themes
            .get(theme.unwrap_or("Solarized (dark)"))
            .ok_or_else(|| Error::NotFound(String::from("could not find theme")))?;

        let mut highligher = HighlightLines::new(syntax, theme);
        let mut s = String::new();
        for line in LinesWithEndings::from(text) {
            // LinesWithEndings enables use of newlines mode
            let ranges: Vec<(Style, &str)> = highligher.highlight_line(line, &syntax_set).unwrap();
            let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
            s.push_str(&escaped);
        }

        Ok(s)
    }
}
