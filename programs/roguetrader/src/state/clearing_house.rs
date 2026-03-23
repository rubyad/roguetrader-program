use anchor_lang::prelude::*;

#[account]
pub struct ClearingHouseState {
    // Authority & roles
    pub authority: Pubkey,              // 32 — admin
    pub settler: Pubkey,                // 32 — settler signer

    // Fee configuration (BPS, max 100 each, total max 1000)
    pub deposit_fee_bps: u16,           // 2 — 100 = 1%
    pub withdrawal_fee_bps: u16,        // 2 — 100 = 1%

    // Fee split (must sum to deposit_fee_bps / withdrawal_fee_bps)
    pub referral_bps: u16,              // 2 — 20 = 0.2%
    pub tier2_referral_bps: u16,        // 2 — 10 = 0.1%
    pub bonus_bps: u16,                 // 2 — 10 = 0.1%
    pub nft_reward_bps: u16,            // 2 — 20 = 0.2%
    pub platform_fee_bps: u16,          // 2 — 40 = 0.4%

    // Fee recipient wallets
    pub platform_wallet: Pubkey,        // 32
    pub bonus_wallet: Pubkey,           // 32
    pub nft_rewarder: Pubkey,           // 32

    // Global state
    pub paused: bool,                   // 1
    pub next_bet_id: u64,              // 8
    pub total_bets_proposed: u64,      // 8
    pub total_bets_settled: u64,       // 8
    pub total_volume: u64,             // 8
    pub total_deposit_fees: u64,       // 8
    pub total_withdrawal_fees: u64,    // 8
    pub total_referral_paid: u64,      // 8
    pub total_nft_rewards_paid: u64,   // 8
    pub total_platform_fees_paid: u64, // 8
    pub total_bonus_paid: u64,         // 8

    // Master vault — single PDA holding ALL SOL for all 30 bots
    pub vault: Pubkey,                 // 32
    pub vault_bump: u8,                // 1

    // ALT address for counterparty batching
    pub vault_lookup_table: Pubkey,    // 32

    pub bump: u8,                       // 1

    // Odds clamp — configurable min/max for compute_odds (bps, 0-10000)
    pub min_odds_bps: u16,             // 2 — e.g. 4500 = 45%
    pub max_odds_bps: u16,             // 2 — e.g. 5500 = 55%

    // Rolling window size for dynamic odds (1-100, 0 = use default 10)
    pub odds_window_size: u8,          // 1 — configurable via update_config

    // If true, winning bots get better odds (inverted formula)
    pub invert_odds: bool,             // 1 — false = normal (winners penalized), true = inverted (winners rewarded)

    // Spread portion that stays in vault (benefits LP holders)
    pub spread_to_lp_bps: u16,        // 2 — e.g. 50 = 0.5% of deposit/withdraw stays as LP NAV boost

    // Max counterparty exposure per bet (bps of each CP's free capital)
    pub max_cp_exposure_bps: u16,     // 2 — 100 = 1%, 0 = disabled (no cap)

    // Stale bet expiry buffer in seconds (0 = use default 120)
    pub stale_bet_buffer_secs: i64,   // 8 — configurable via update_config

    // L-6: Granular pause flags (deposits_paused reuses the `paused` byte position concept
    // but is a separate field stored after stale_bet_buffer_secs)
    pub deposits_paused: bool,        // 1
    pub withdrawals_paused: bool,     // 1
    pub betting_paused: bool,         // 1

    // L-4: Two-step authority transfer
    pub pending_authority: Pubkey,    // 32 — Pubkey::default() = no pending transfer

    pub _reserved: [u8; 74],          // 74 (was 110, used 3 for pause flags + 32 for pending_authority + 1 padding)
}
// Total: ~585 bytes (unchanged — consumed from _reserved)
