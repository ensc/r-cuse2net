#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Nix(#[from] nix::Error),

    #[error(transparent)]
    Cuse(#[from] ensc_cuse_ffi::Error),

    #[error(transparent)]
    Protocol(#[from] crate::proto::Error),

    #[error("remote error {0}")]
    Remote(i32),
}
