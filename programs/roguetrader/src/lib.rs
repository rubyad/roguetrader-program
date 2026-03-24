use anchor_lang::prelude::*;

#[cfg(not(feature = "devnet"))]
declare_id!("EDSh6vJ7KDsB6UStKYt4mDBcAJqVtS7JWoPbDXw81LSr");

#[cfg(feature = "devnet")]
declare_id!("EDb1DSJZZuRrwnaFbo3oRStNemcbpZd2VxdYjvMP3fJt");

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "RogueTrader",
    project_url: "https://roguetrader.io",
    contacts: "email:info@roguetrader.io",
    policy: "https://roguetrader.io/security",
    preferred_languages: "en",
    source_code: "https://github.com/roguetrader-io/roguetrader-program"
}

pub mod state;
pub mod instructions;
pub mod errors;
pub mod events;
pub mod pyth;

use instructions::*;

#[program]
pub mod roguetrader {
    use super::*;

    // ========================================================================
    // Admin Instructions
    // ========================================================================

    /// Initialize ClearingHouseState + master vault PDA
    pub fn initialize(
        ctx: Context<Initialize>,
        deposit_fee_bps: u16,
        withdrawal_fee_bps: u16,
        referral_bps: u16,
        tier2_referral_bps: u16,
        bonus_bps: u16,
        nft_reward_bps: u16,
        platform_fee_bps: u16,
        spread_to_lp_bps: u16,
    ) -> Result<()> {
        instructions::initialize::handler(
            ctx,
            deposit_fee_bps,
            withdrawal_fee_bps,
            referral_bps,
            tier2_referral_bps,
            bonus_bps,
            nft_reward_bps,
            platform_fee_bps,
            spread_to_lp_bps,
        )
    }

    /// Create AgentVault + LP mint + LP authority for a bot
    pub fn create_agent_vault(
        ctx: Context<CreateAgentVault>,
        bot_id: u8,
        group_id: u8,
        name: [u8; 16],
    ) -> Result<()> {
        instructions::create_agent_vault::handler(ctx, bot_id, group_id, name)
    }

    /// Create GroupConfig with Pyth feed assignments
    pub fn create_group_config(
        ctx: Context<CreateGroupConfig>,
        group_id: u8,
        name: [u8; 32],
        pyth_feeds: Vec<Pubkey>,
    ) -> Result<()> {
        instructions::create_group_config::handler(ctx, group_id, name, pyth_feeds)
    }

    /// Authority deposits SOL into master vault, increments bot's sol_balance
    pub fn fund_vault(
        ctx: Context<FundVault>,
        bot_id: u8,
        amount: u64,
    ) -> Result<()> {
        instructions::fund_vault::handler(ctx, bot_id, amount)
    }

    /// Update fee rates, wallets, settler
    pub fn update_config(
        ctx: Context<UpdateConfig>,
        deposit_fee_bps: Option<u16>,
        withdrawal_fee_bps: Option<u16>,
        referral_bps: Option<u16>,
        tier2_referral_bps: Option<u16>,
        bonus_bps: Option<u16>,
        nft_reward_bps: Option<u16>,
        platform_fee_bps: Option<u16>,
        platform_wallet: Option<Pubkey>,
        bonus_wallet: Option<Pubkey>,
        nft_rewarder: Option<Pubkey>,
        settler: Option<Pubkey>,
        vault_lookup_table: Option<Pubkey>,
        min_odds_bps: Option<u16>,
        max_odds_bps: Option<u16>,
        odds_window_size: Option<u8>,
        invert_odds: Option<bool>,
        spread_to_lp_bps: Option<u16>,
        max_cp_exposure_bps: Option<u16>,
        stale_bet_buffer_secs: Option<i64>,
    ) -> Result<()> {
        instructions::update_config::handler(
            ctx,
            deposit_fee_bps,
            withdrawal_fee_bps,
            referral_bps,
            tier2_referral_bps,
            bonus_bps,
            nft_reward_bps,
            platform_fee_bps,
            platform_wallet,
            bonus_wallet,
            nft_rewarder,
            settler,
            vault_lookup_table,
            min_odds_bps,
            max_odds_bps,
            odds_window_size,
            invert_odds,
            spread_to_lp_bps,
            max_cp_exposure_bps,
            stale_bet_buffer_secs,
        )
    }

    /// Add/remove Pyth feeds from a group
    pub fn update_group_feeds(
        ctx: Context<UpdateGroupFeeds>,
        group_id: u8,
        pyth_feeds: Vec<Pubkey>,
        feed_count: u8,
    ) -> Result<()> {
        instructions::update_group_feeds::handler(ctx, group_id, pyth_feeds, feed_count)
    }

    /// Emergency pause all operations (with optional granular control)
    pub fn pause(
        ctx: Context<Pause>,
        paused: bool,
        deposits_paused: Option<bool>,
        withdrawals_paused: Option<bool>,
        betting_paused: Option<bool>,
    ) -> Result<()> {
        instructions::pause::handler(ctx, paused, deposits_paused, withdrawals_paused, betting_paused)
    }

    // ========================================================================
    // User Instructions
    // ========================================================================

    /// User deposits SOL, receives bot's LP tokens
    pub fn deposit_sol(ctx: Context<DepositSol>, amount: u64) -> Result<()> {
        instructions::deposit_sol::handler(ctx, amount)
    }

    /// User burns LP tokens, receives SOL
    pub fn withdraw_sol(ctx: Context<WithdrawSol>, lp_amount: u64) -> Result<()> {
        instructions::withdraw_sol::handler(ctx, lp_amount)
    }

    /// User sets their referrer wallet
    pub fn set_referrer(ctx: Context<SetReferrer>) -> Result<()> {
        instructions::set_referrer::handler(ctx)
    }

    // ========================================================================
    // Bot Instructions (settler-signed)
    // ========================================================================

    /// Bot proposes a bet, locks capital from all counterparties
    pub fn propose_bet<'info>(
        ctx: Context<'_, '_, 'info, 'info, ProposeBet<'info>>,
        direction: u8,
        stake_bps: u64,
        duration_seconds: i64,
    ) -> Result<()> {
        instructions::propose_bet::handler(ctx, direction, stake_bps, duration_seconds)
    }

    /// Settle an expired bet, redistribute sol_balance
    pub fn settle_bet<'info>(
        ctx: Context<'_, '_, 'info, 'info, SettleBet<'info>>,
    ) -> Result<()> {
        instructions::settle_bet::handler(ctx)
    }

    /// Close a settled bet account to reclaim rent
    pub fn close_bet(ctx: Context<CloseBet>) -> Result<()> {
        instructions::close_bet::handler(ctx)
    }

    /// Settler-signed referral setting (seamless flow)
    pub fn admin_set_referrer(
        ctx: Context<AdminSetReferrer>,
        player_key: Pubkey,
        referrer_key: Pubkey,
    ) -> Result<()> {
        instructions::admin_set_referrer::handler(ctx, player_key, referrer_key)
    }

    /// Authority expires stuck bets past expiry + buffer
    pub fn expire_stale_bet<'info>(
        ctx: Context<'_, '_, 'info, 'info, ExpireStaleBet<'info>>,
    ) -> Result<()> {
        instructions::expire_stale_bet::handler(ctx)
    }

    /// Create LP token metadata via Metaplex for a bot's LP mint
    pub fn create_lp_metadata(
        ctx: Context<CreateLpMetadata>,
        bot_id: u8,
        name: String,
        symbol: String,
        uri: String,
    ) -> Result<()> {
        instructions::create_lp_metadata::handler(ctx, bot_id, name, symbol, uri)
    }

    /// Update LP token metadata (name, symbol, uri) via Metaplex
    pub fn update_lp_metadata(
        ctx: Context<UpdateLpMetadata>,
        bot_id: u8,
        name: String,
        symbol: String,
        uri: String,
    ) -> Result<()> {
        instructions::update_lp_metadata::handler(ctx, bot_id, name, symbol, uri)
    }

    /// Authority resets a vault's active_bet_count, locked_sol, and counterparty_locked_sol (safety valve)
    pub fn admin_reset_vault(
        ctx: Context<AdminResetVault>,
        active_bet_count: u8,
        locked_sol: u64,
        counterparty_locked_sol: u64,
    ) -> Result<()> {
        instructions::admin_reset_vault::handler(ctx, active_bet_count, locked_sol, counterparty_locked_sol)
    }

    /// L-4: Current authority proposes a new authority (step 1 of 2)
    pub fn propose_authority_transfer(ctx: Context<ProposeAuthorityTransfer>) -> Result<()> {
        instructions::transfer_authority::propose_handler(ctx)
    }

    /// L-4: New authority accepts the transfer (step 2 of 2)
    pub fn accept_authority_transfer(ctx: Context<AcceptAuthorityTransfer>) -> Result<()> {
        instructions::transfer_authority::accept_handler(ctx)
    }
}
