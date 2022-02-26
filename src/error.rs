use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Custom({0})")]
    Custom(String),
}
