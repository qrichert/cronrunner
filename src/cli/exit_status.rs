use std::process::{ExitCode, Termination};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ExitStatus {
    Success,
    Failure,
    ArgsError,
    Error(u8),
}

impl From<u8> for ExitStatus {
    fn from(code: u8) -> Self {
        match code {
            0 => Self::Success,
            1 => Self::Failure,
            2 => Self::ArgsError,
            code => Self::Error(code),
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

// `ExitCode` is not `PartialEq`
#[cfg(not(tarpaulin_include))]
impl Termination for ExitStatus {
    fn report(self) -> ExitCode {
        match self {
            Self::Success => ExitCode::SUCCESS,
            Self::Failure => ExitCode::FAILURE,
            Self::ArgsError => ExitCode::from(2),
            Self::Error(code) => ExitCode::from(code),
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
        assert_eq!(ExitStatus::from(255i32), ExitStatus::Error(255u8));
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
