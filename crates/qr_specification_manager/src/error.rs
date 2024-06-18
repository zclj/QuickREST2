pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    OpenAPIFetchFailed(qr_specification_resource_access::Error),

    OpenAPIParseFailed(qr_open_api::Error),
}

impl From<qr_specification_resource_access::Error> for Error {
    fn from(val: qr_specification_resource_access::Error) -> Self {
        Self::OpenAPIFetchFailed(val)
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl std::error::Error for Error {}
