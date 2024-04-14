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

use std::process::{Command, Output};

/// Low level detail about the error.
///
/// This is only meant to be used attached to a [`ReadError`], provided
/// by [`Reader`].
#[derive(Debug)]
pub enum ReadErrorDetail {
    /// If the command succeeded with a non-zero exit code.
    NonZeroExit {
        /// The exit code or `None` if the process was killed early.
        exit_code: Option<i32>,
        /// Standard error or `None` if empty.
        stderr: Option<String>,
    },
    /// If the command failed to execute at all (e.g., `crontab`
    /// executable not found).
    CouldNotRunCommand,
}

/// Additional context, provided by [`Reader`] in case of an error.
#[derive(Debug)]
pub struct ReadError {
    /// Explanation of the error in plain English.
    pub reason: String,
    /// Detail about the error. May contain exit code and stderr, see
    /// [`ReadErrorDetail`].
    pub detail: ReadErrorDetail,
}

/// Read current user's `crontab`.
///
/// [`Reader`] only provides the [`read()`](Reader::read()) function
/// that outputs a `String` or a [`ReadError`].
///
/// The `String` result can be fed to
/// [`Parser::parse()`](super::Parser::parse()) for lexing and parsing.
pub struct Reader;

impl Reader {
    /// Read current user's `crontab` to a `String`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cronrunner::crontab::Reader;
    ///
    /// let crontab: String = match Reader::read() {
    ///     Ok(crontab) => crontab, // Output of `crontab -l` as string.
    ///     Err(_) => return (),
    /// };
    /// ```
    ///
    /// # Errors
    ///
    /// Will return [`Err(ReadError)`](ReadError) if the `crontab`
    /// cannot be read. This can happen when:
    ///
    /// - The `crontab -l` command returns with a non-zero exit code or
    ///   no exit code at all (process terminated).
    /// - The `crontab` command fails (e.g., executable not found).
    pub fn read() -> Result<String, ReadError> {
        let output = Command::new("crontab").arg("-l").output();
        match output {
            Ok(output) => Self::handle_output_ok(&output),
            Err(_) => Self::handle_output_err(),
        }
    }

    // `Ok` means that there was no critical error and the executable could
    // be run, NOT that the process exited with exit code 0.
    fn handle_output_ok(output: &Output) -> Result<String, ReadError> {
        if output.status.success() {
            // Exit 0
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            // Not exit 0 (e.g., 'crontab --option_does_not_exist', etc.)
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

            Err(ReadError {
                reason: String::from("Cannot read crontab of current user."),
                detail: ReadErrorDetail::NonZeroExit {
                    exit_code: output.status.code(),
                    stderr: if stderr.is_empty() {
                        None
                    } else {
                        Some(stderr)
                    },
                },
            })
        }
    }

    /// `Err` means a critical error happened, like for example the
    /// executable is missing.
    fn handle_output_err() -> Result<String, ReadError> {
        Err(ReadError {
            reason: String::from("Unable to locate crontab executable on the system."),
            detail: ReadErrorDetail::CouldNotRunCommand,
        })
    }
}
