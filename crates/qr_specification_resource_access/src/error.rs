pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    OpenAPIFetchFromURLFailed(reqwest::Error),

    OpenAPIInvalidJSON(serde_json::Error),

    OpenAPIInvalidFormat(String),

    OpenAPIReadFileFailure(std::io::Error),
}

impl From<reqwest::Error> for Error {
    fn from(val: reqwest::Error) -> Self {
        Self::OpenAPIFetchFromURLFailed(val)
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl std::error::Error for Error {}
