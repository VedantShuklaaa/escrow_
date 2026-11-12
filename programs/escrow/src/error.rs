use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    #[msg("Amount must be greater than zero")]
    InvalidAmount,
    #[msg("Mint A and mint B must differ")]
    SameMint,
    #[msg("Vault is empty")]
    EmptyVault,
}
