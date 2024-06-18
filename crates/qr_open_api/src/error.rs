pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    OpenAPIReadFileFailure(std::io::Error),

    OpenAPIParseFailure(String),
}
