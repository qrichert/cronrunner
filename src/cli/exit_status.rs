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

use std::process::{ExitCode, Termination};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ExitStatus {
    Success,
    Failure,
    ArgsError,
    Code(u8),
}

impl From<u8> for ExitStatus {
    fn from(code: u8) -> Self {
        match code {
            0 => Self::Success,
            1 => Self::Failure,
            2 => Self::ArgsError,
            code => Self::Code(code),
        }
    }
}

impl From<i32> for ExitStatus {
    fn from(code: i32) -> Self {
        // code in [0 ; 255]
        if code >= i32::from(u8::MIN) && code <= i32::from(u8::MAX) {
            u8::try_from(code).expect("bounds have been checked").into()
        } else {
            Self::Failure // Default to generic exit 1.
        }
    }
}

// `ExitCode` is not `PartialEq`.
#[cfg(not(tarpaulin_include))]
impl Termination for ExitStatus {
    fn report(self) -> ExitCode {
        match self {
            Self::Success => ExitCode::SUCCESS,
            Self::Failure => ExitCode::FAILURE,
            Self::ArgsError => ExitCode::from(2),
            Self::Code(code) => ExitCode::from(code),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_i32_to_exit_status() {
        // Test boundaries and special cases.
        assert_eq!(ExitStatus::from(0i32), ExitStatus::Success);
        assert_eq!(ExitStatus::from(1i32), ExitStatus::Failure);
        assert_eq!(ExitStatus::from(2i32), ExitStatus::ArgsError);
        assert_eq!(ExitStatus::from(255i32), ExitStatus::Code(255u8));
    }

    #[test]
    fn convert_i32_to_exit_status_out_of_lower_bound() {
        assert_eq!(ExitStatus::from(-1i32), ExitStatus::Failure);
    }

    #[test]
    fn convert_i32_to_exit_status_out_of_upper_bound() {
        assert_eq!(ExitStatus::from(256i32), ExitStatus::Failure);
    }
}
