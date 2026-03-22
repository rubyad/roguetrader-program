use anchor_lang::prelude::*;

#[account]
pub struct PlayerState {
    pub wallet: Pubkey,                 // 32
    pub referrer: Pubkey,              // 32 — Pubkey::default() if none
    pub tier2_referrer: Pubkey,        // 32 — Pubkey::default() if none
    pub total_deposited: u64,          // 8
    pub total_withdrawn: u64,          // 8
    pub deposit_count: u64,            // 8
    pub withdrawal_count: u64,         // 8
    pub bump: u8,                       // 1
    pub _reserved: [u8; 64],           // 64
}
// Total: ~193 bytes
