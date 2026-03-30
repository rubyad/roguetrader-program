use anchor_lang::prelude::*;

// ============================================================================
// Admin Events
// ============================================================================

#[event]
pub struct ClearingHouseInitialized {
    pub authority: Pubkey,
    pub settler: Pubkey,
    pub vault: Pubkey,
    pub deposit_fee_bps: u16,
    pub withdrawal_fee_bps: u16,
    pub timestamp: i64,
}

#[event]
pub struct AgentVaultCreated {
    pub bot_id: u8,
    pub group_id: u8,
    pub name: [u8; 16],
    pub lp_mint: Pubkey,
    pub vault_pubkey: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct GroupConfigCreated {
    pub group_id: u8,
    pub name: [u8; 32],
    pub feed_count: u8,
    pub timestamp: i64,
}

#[event]
pub struct VaultFunded {
    pub bot_id: u8,
    pub amount: u64,
    pub new_balance: u64,
    pub timestamp: i64,
}

#[event]
pub struct ProtocolPaused {
    pub authority: Pubkey,
    pub paused: bool,
    pub timestamp: i64,
}

#[event]
pub struct ConfigUpdated {
    pub field_id: u8,
    pub old_value: u64,
    pub new_value: u64,
    pub authority: Pubkey,
    pub timestamp: i64,
}

// ============================================================================
// User Events
// ============================================================================

#[event]
pub struct DepositCompleted {
    pub depositor: Pubkey,
    pub bot_id: u8,
    pub sol_amount: u64,
    pub fee_amount: u64,
    pub lp_minted: u64,
    pub new_sol_balance: u64,
    pub new_lp_supply: u64,
    pub timestamp: i64,
}

#[event]
pub struct WithdrawCompleted {
    pub withdrawer: Pubkey,
    pub bot_id: u8,
    pub lp_burned: u64,
    pub sol_returned: u64,
    pub fee_amount: u64,
    pub new_sol_balance: u64,
    pub new_lp_supply: u64,
    pub timestamp: i64,
}

#[event]
pub struct ReferrerSet {
    pub player: Pubkey,
    pub referrer: Pubkey,
    pub tier2_referrer: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct FeePaid {
    pub fee_type: u8, // 0=tier1_referral, 1=tier2_referral, 2=bonus, 3=nft_reward, 4=platform
    pub recipient: Pubkey,
    pub amount: u64,
    pub bot_id: u8,
    pub timestamp: i64,
}

// ============================================================================
// Bet Events
// ============================================================================

#[event]
pub struct BetProposed {
    pub bet_id: u64,
    pub proposer_bot: u8,
    pub pyth_feed: Pubkey,
    pub direction: u8,
    pub proposer_stake: u64,
    pub counterparty_pool: u64,
    pub win_rate_bps: u16,
    pub entry_price: i64,
    pub entry_conf: u64,
    pub duration_seconds: i64,
    pub expiry_timestamp: i64,
    pub timestamp: i64,
}

#[event]
pub struct BetSettled {
    pub bet_id: u64,
    pub proposer_bot: u8,
    pub outcome: u8,
    pub entry_price: i64,
    pub exit_price: i64,
    pub proposer_stake: u64,
    pub counterparty_pool: u64,
    pub tax_amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct BetClosed {
    pub bet_id: u64,
    pub rent_returned_to: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct StaleBetExpired {
    pub bet_id: u64,
    pub proposer_bot: u8,
    pub locked_sol_returned: u64,
    pub timestamp: i64,
}

// ============================================================================
// Security Audit Events
// ============================================================================

/// M-1: Emitted when a fee transfer CPI fails (fee stays in vault)
#[event]
pub struct FeeTransferFailed {
    pub recipient: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

/// M-9: Emitted when a Pubkey config field changes (settler, wallets, ALT)
#[event]
pub struct ConfigPubkeyUpdated {
    pub field_id: u8,
    pub old_value: Pubkey,
    pub new_value: Pubkey,
    pub authority: Pubkey,
    pub timestamp: i64,
}

/// L-4: Emitted when authority is transferred
#[event]
pub struct AuthorityTransferred {
    pub old_authority: Pubkey,
    pub new_authority: Pubkey,
    pub timestamp: i64,
}

/// Emitted when a raffle is drawn
#[event]
pub struct RaffleDrawn {
    pub raffle_number: u64,
    pub winner_bot_id: u8,
    pub reward_amount: u64,
    pub slot: u64,
    pub timestamp: i64,
    pub total_weight: u64,
}

/// L-6: Emitted when granular pause state changes
#[event]
pub struct PauseStateChanged {
    pub authority: Pubkey,
    pub deposits_paused: bool,
    pub withdrawals_paused: bool,
    pub betting_paused: bool,
    pub timestamp: i64,
}
