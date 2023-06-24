#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Nix(#[from] nix::Error),
}
