#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Nix(#[from] nix::Error),
    #[error("bad alignment of {0} bytes")]
    Alignment(usize),
    #[error("not large enough; {0} bytes missing")]
    Size(usize),
    #[error("eof reached")]
    Eof,
    #[error("truncation error ({0}, {1})")]
    BadTruncate(usize, usize),
    #[error("transmitted only {0} bytes from expected {1} ones")]
    BadSend(usize, usize)
}
