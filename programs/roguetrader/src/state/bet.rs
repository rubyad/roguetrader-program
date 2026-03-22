use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
pub struct CounterpartyPosition {
    pub bot_id: u8,                     // 1
    pub stake: u64,                     // 8
}
// 9 bytes each

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq)]
pub enum Direction {
    Long,
    Short,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Long
    }
}

/// Bet outcome: 0=pending, 1=proposer_won, 2=proposer_lost, 3=tie
pub const OUTCOME_PENDING: u8 = 0;
pub const OUTCOME_PROPOSER_WON: u8 = 1;
pub const OUTCOME_PROPOSER_LOST: u8 = 2;
pub const OUTCOME_TIE: u8 = 3;

pub const MAX_COUNTERPARTIES: usize = 29;

#[account]
pub struct Bet {
    pub bet_id: u64,                    // 8
    pub proposer_bot: u8,              // 1
    pub pyth_feed: Pubkey,             // 32
    pub direction: Direction,           // 1
    pub duration_seconds: i64,         // 8

    // Stakes
    pub proposer_stake: u64,           // 8
    pub counterparty_pool: u64,        // 8
    pub win_rate_bps_at_open: u16,     // 2 — odds snapshot

    // Counterparty breakdown (fixed 29 slots, cp_count tells how many are active)
    pub counterparties: [CounterpartyPosition; 29], // 261
    pub cp_count: u8,                  // 1

    // Price data
    pub entry_price: i64,              // 8 — Pyth price
    pub entry_conf: u64,               // 8 — Pyth confidence
    pub entry_expo: i32,               // 4 — Pyth exponent
    pub entry_timestamp: i64,          // 8
    pub expiry_timestamp: i64,         // 8

    // Settlement
    pub settled: bool,                 // 1
    pub outcome: u8,                   // 1 — 0=pending, 1=proposer_won, 2=proposer_lost, 3=tie
    pub exit_price: i64,               // 8
    pub exit_conf: u64,                // 8
    pub settle_timestamp: i64,         // 8

    pub bump: u8,                       // 1
    pub _reserved: [u8; 32],           // 32
}
// Total: ~420 bytes
// Rent: ~0.003 SOL

/// Stale bet buffer in seconds (5 minutes)
pub const STALE_BET_BUFFER: i64 = 300;
