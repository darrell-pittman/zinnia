use alsa::Error as AlsaError;
use std::{error::Error as StdError, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Alsa,
    Zinnia,
}

impl StdError for Kind {}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Kind::Alsa => write!(f, "Alsa Error"),
            Kind::Zinnia => write!(f, "Zinnia Error"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub struct Error(&'static str, Kind);

impl From<AlsaError> for Error {
    fn from(e: AlsaError) -> Self {
        Error(e.func(), Kind::Alsa)
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        "ZINNIA error"
    }
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.1)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ZINNIA function '{}' failed with error '{}'",
            self.0, self.1
        )
    }
}
