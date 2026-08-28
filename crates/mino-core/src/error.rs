use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("unknown tweak `{0}`")]
    UnknownTweak(String),

    #[error("tweak `{tweak}` is not available on this Windows build: {reason}")]
    Unsupported { tweak: String, reason: String },

    #[error("invalid value for `{tweak}`: got {got}, expected {expected}")]
    BadValue {
        tweak: String,
        got: String,
        expected: String,
    },

    /// The registry held something we did not expect (wrong type, or a value
    /// outside the range we know how to map). We refuse to guess: a tweak that
    /// cannot read the current state cannot promise a clean revert.
    #[error("unexpected registry state at {loc}: {detail}")]
    UnexpectedState { loc: String, detail: String },

    /// Raised by a provider. The concrete OS error is flattened to a string so
    /// this crate stays free of platform dependencies.
    #[error("registry: {0}")]
    Registry(String),

    #[error("shell refresh: {0}")]
    Shell(String),

    #[error("this change needs administrator rights ({0}) and the broker is not available yet")]
    NeedsElevation(String),

    #[error("journal: {0}")]
    Journal(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Error {
    pub fn registry(detail: impl Into<String>) -> Self {
        Error::Registry(detail.into())
    }

    pub fn shell(detail: impl Into<String>) -> Self {
        Error::Shell(detail.into())
    }
}
