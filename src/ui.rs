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

// TODO: Colors as constants, so they can be used inside --help examples
//  pub const ERROR: &str = "\x1b[0;91m";
//  pub const HIGHLIGHT: &str = "\x1b[0;92m";
//  pub const ATTENUATE: &str = "\x1b[0;90m";
//  pub const TITLE: &str = "\x1b[97;1;4m";
//  pub const RESET: &str = "\x1b[0m";

#[must_use]
pub fn color_error(string: &str) -> String {
    format!("\x1b[0;91m{string}\x1b[0m")
}

#[must_use]
pub fn color_highlight(string: &str) -> String {
    format!("\x1b[0;92m{string}\x1b[0m")
}

#[must_use]
pub fn color_attenuate(string: &str) -> String {
    format!("\x1b[0;90m{string}\x1b[0m")
}

#[must_use]
pub fn color_title(string: &str) -> String {
    format!("\x1b[1;4;97m{string}\x1b[0m")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_error_is_red() {
        assert_eq!(
            color_error("this is marked as error"),
            "\x1b[0;91mthis is marked as error\x1b[0m"
        );
    }

    #[test]
    fn color_highlight_is_green() {
        assert_eq!(
            color_highlight("this is highlighted"),
            "\x1b[0;92mthis is highlighted\x1b[0m"
        );
    }

    #[test]
    fn color_attenuate_is_grey() {
        assert_eq!(
            color_attenuate("this is attenuated"),
            "\x1b[0;90mthis is attenuated\x1b[0m"
        );
    }

    #[test]
    fn color_title_is_white_bold_underlined() {
        assert_eq!(
            color_title("this is white, bold, and underlined"),
            "\x1b[1;4;97mthis is white, bold, and underlined\x1b[0m"
        );
    }
}
