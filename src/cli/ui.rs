// cronrunner — Run cron jobs manually.
// Copyright (C) 2024  Quentin Richert
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::env;
use std::sync::LazyLock;

/// `true` if `NO_COLOR` is set and is non-empty.
#[cfg(not(tarpaulin_include))]
#[allow(unreachable_code)]
pub static NO_COLOR: LazyLock<bool> = LazyLock::new(|| {
    #[cfg(test)]
    {
        return false;
    }
    // Contrary to `env::var()`, `env::var_os()` does not require the
    // value to be valid Unicode.
    match env::var_os("NO_COLOR") {
        Some(value) => !value.is_empty(),
        None => false,
    }
});

pub const ERROR: &str = "\x1b[0;91m";
pub const HIGHLIGHT: &str = "\x1b[0;92m";
pub const ATTENUATE: &str = "\x1b[0;90m";
pub const TITLE: &str = "\x1b[1;4m";
pub const RESET: &str = "\x1b[0m";

pub struct Color;

impl Color {
    #[must_use]
    pub fn error(string: &str) -> String {
        Self::color(ERROR, string)
    }

    #[must_use]
    pub fn highlight(string: &str) -> String {
        Self::color(HIGHLIGHT, string)
    }

    #[must_use]
    pub fn attenuate(string: &str) -> String {
        Self::color(ATTENUATE, string)
    }

    #[must_use]
    pub fn title(string: &str) -> String {
        Self::color(TITLE, string)
    }

    #[must_use]
    fn color(color: &str, string: &str) -> String {
        if *NO_COLOR {
            #[cfg(not(tarpaulin_include))] // Unreachable in tests.
            return string.into();
        }
        format!("{color}{string}{RESET}")
    }

    #[must_use]
    pub fn maybe_color(color: &str) -> &str {
        if *NO_COLOR {
            #[cfg(not(tarpaulin_include))] // Unreachable in tests.
            return "";
        }
        color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_error_is_red() {
        assert_eq!(
            Color::error("this is marked as error"),
            "\x1b[0;91mthis is marked as error\x1b[0m"
        );
    }

    #[test]
    fn color_highlight_is_green() {
        assert_eq!(
            Color::highlight("this is highlighted"),
            "\x1b[0;92mthis is highlighted\x1b[0m"
        );
    }

    #[test]
    fn color_attenuate_is_grey() {
        assert_eq!(
            Color::attenuate("this is attenuated"),
            "\x1b[0;90mthis is attenuated\x1b[0m"
        );
    }

    #[test]
    fn color_title_is_bold_underlined() {
        assert_eq!(
            Color::title("this is bold, and underlined"),
            "\x1b[1;4mthis is bold, and underlined\x1b[0m"
        );
    }
}
