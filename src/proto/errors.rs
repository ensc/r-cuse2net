#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("payload too large {0}")]
    PayloadTooLarge(usize),

    #[error("unsupported op {0}")]
    BadOp(u8),
}
