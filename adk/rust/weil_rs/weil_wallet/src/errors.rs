use thiserror::Error;

#[derive(Debug, Error)]
#[error("invalid contract id: {}", self.msg)]
pub struct InvalidContractIdError {
    msg: String,
}
