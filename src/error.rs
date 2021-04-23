use alsa::Error as AlsaError;
use std::{
    error::Error as StdError,
    fmt,
    sync::mpsc::{RecvError, SendError},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Alsa,
    Zinnia,
    Channel,
    Poll,
}

impl StdError for Kind {}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Kind::Alsa => write!(f, "Alsa Error"),
            Kind::Zinnia => write!(f, "Zinnia Error"),
            Kind::Channel => write!(f, "Channel Error"),
            Kind::Poll => write!(f, "Poll Error"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub struct Error(&'static str, Kind);

impl Error {
    pub fn new(func: &'static str, kind: Kind) -> Error {
        Error(func, kind)
    }

    pub fn kind(&self) -> Kind {
        self.1
    }
}

impl From<AlsaError> for Error {
    fn from(e: AlsaError) -> Self {
        Error(e.func(), Kind::Alsa)
    }
}

impl<T> From<SendError<T>> for Error {
    fn from(_: SendError<T>) -> Self {
        Error("Send Error", Kind::Channel)
    }
}

impl From<RecvError> for Error {
    fn from(_: RecvError) -> Self {
        Error("Receive Error", Kind::Channel)
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
