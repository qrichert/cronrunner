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

#![cfg(not(tarpaulin_include))]

//! Output text through a pager.
//!
//! It uses `less` by default, or any pager set by the `PAGER`
//! environment variable.
//!
//! The point of interest is the [`Pager`] struct.
//!
//! # Examples
//!
//! ```no_run
//! use crate::cli::output::Pager;
//!
//! // If pager fails, fall back to printing text.
//! Pager::page_or_print("very long text");
//! ```

use std::env;
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::sync::LazyLock;

/// Pager to use, lazily determined.
///
/// The logic is as follows:
///
/// 1. Look for `PAGER` in the environment.
/// 2. If not set, default to `less`.
pub static PAGER: LazyLock<String> =
    LazyLock::new(|| env::var("PAGER").unwrap_or_else(|_| String::from("less")));

/// Output text through a pager.
pub struct Pager;

impl Pager {
    /// Output `content` with default pager or print to stdout on error.
    ///
    /// This is a helper function for the common case where you don't
    /// really care whether the pager succeeded or not. Worst case
    /// scenario just print to stdout, no big deal.
    pub fn page_or_print(content: &str) {
        if Self::page(content).is_err() {
            if content.ends_with('\n') {
                print!("{content}");
            } else {
                println!("{content}");
            }
        }
    }

    /// Try to use default pager to output `content`.
    ///
    /// The pager is read from the `PAGER` environment variable, or
    /// defaults to `less`.
    ///
    /// # Errors
    ///
    /// Errors if the pager cannot be spawned (e.g., executable
    /// missing), or stdin cannot be captured or written to.
    pub fn page(content: &str) -> Result<(), io::Error> {
        let mut pager = Command::new(&*PAGER);
        pager.stdin(Stdio::piped());
        pager.stdout(Stdio::inherit());
        pager.stderr(Stdio::inherit());

        if *PAGER == "less" || PAGER.ends_with("/less") {
            pager.env("LESSCHARSET", "UTF-8");
            // Use short args for better compatibility.
            pager.arg("-R"); // `--RAW-CONTROL-CHARS` Do not render ANSI sequences as text.
            pager.arg("-F"); // `--quit-if-one-screen` Do not page if the entire output fits on the screen.
            pager.arg("-X"); // `--no-init` Leave content on screen after exit.
        }

        let mut child = pager.spawn()?;

        let Some(stdin) = child.stdin.as_mut() else {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Failed to open stdin.",
            ));
        };

        if content.ends_with('\n') {
            write!(stdin, "{content}")?;
        } else {
            writeln!(stdin, "{content}")?;
        }

        child.wait()?;

        Ok(())
    }
}
