use anchor_lang::prelude::*;

#[account]
pub struct ReferralState {
    pub referrer: Pubkey,               // 32
    pub total_earnings: u64,           // 8
    pub referral_count: u64,           // 8
    pub bump: u8,                       // 1
    pub _reserved: [u8; 64],           // 64
}
// Total: ~113 bytes
