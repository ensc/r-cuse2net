use super::Sequence;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("payload too large {0}")]
    PayloadTooLarge(usize),

    #[error("unsupported op {0}")]
    BadOp(u8),

    #[error("unsupported response")]
    BadResponse,

    #[error("unsupported request")]
    BadRequest,

    #[error("bad sequence")]
    BadSequence,

    #[error("remote error {1} on sequence {0:?}")]
    RemoteError(Option<Sequence>, i32),
}
