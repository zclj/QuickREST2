pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    ReportFileFailure(std::io::Error),
    ResultSerializationFailure(serde_json::Error),
}

impl From<std::io::Error> for Error {
    fn from(val: std::io::Error) -> Self {
        Self::ReportFileFailure(val)
    }
}

impl From<serde_json::Error> for Error {
    fn from(val: serde_json::Error) -> Self {
        Self::ResultSerializationFailure(val)
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl std::error::Error for Error {}
