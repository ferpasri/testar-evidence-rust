use std::fmt;

#[derive(Debug)]
pub enum Error {
    InvalidRecord(String),
    Validation {
        code: &'static str,
        field: &'static str,
        message: String,
    },
    Conflict(String),
    UnknownReference(String),
    Io(std::io::Error),
    Json(serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn code(&self) -> &str {
        match self {
            Self::Validation { code, .. } => code,
            Self::InvalidRecord(_) => "invalid_record",
            Self::Conflict(_) => "conflict",
            Self::UnknownReference(_) => "invalid_reference",
            Self::Io(_) => "io_error",
            Self::Json(_) => "invalid_json",
        }
    }

    pub fn field(&self) -> Option<&str> {
        match self {
            Self::Validation { field, .. } => Some(field),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation {
                code,
                field,
                message,
            } => write!(formatter, "{code} at {field}: {message}"),
            Self::InvalidRecord(message) => {
                write!(formatter, "invalid record: {message}")
            }
            Self::Conflict(message) => {
                write!(formatter, "conflicting record: {message}")
            }
            Self::UnknownReference(message) => {
                write!(formatter, "unknown reference: {message}")
            }
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Json(error) => write!(formatter, "JSON error: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}
