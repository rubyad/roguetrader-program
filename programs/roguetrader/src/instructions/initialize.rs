use anchor_lang::prelude::*;
use crate::state::ClearingHouseState;
use crate::errors::RogueTraderError;
use crate::events::ClearingHouseInitialized;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<ClearingHouseState>(),
        seeds = [b"clearing_house"],
        bump,
    )]
    pub clearing_house: Account<'info, ClearingHouseState>,

    /// Master vault PDA — holds ALL SOL for all 30 bots
    /// CHECK: System-owned PDA, validated by seeds
    #[account(
        mut,
        seeds = [b"vault"],
        bump,
    )]
    pub vault: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Just storing the pubkey, no data read
    pub settler: AccountInfo<'info>,

    /// CHECK: Platform fee wallet
    pub platform_wallet: AccountInfo<'info>,

    /// CHECK: Bonus wallet
    pub bonus_wallet: AccountInfo<'info>,

    /// CHECK: NFT rewarder wallet
    pub nft_rewarder: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
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
    // Validate fee bounds — max 1000 bps (10%) each
    require!(deposit_fee_bps <= 1000, RogueTraderError::InvalidConfig);
    require!(withdrawal_fee_bps <= 1000, RogueTraderError::InvalidConfig);
    require!(spread_to_lp_bps <= deposit_fee_bps, RogueTraderError::InvalidConfig);

    // Validate fee split: wallet splits must sum to deposit_fee_bps minus spread_to_lp_bps
    let split_sum = referral_bps
        .checked_add(tier2_referral_bps)
        .and_then(|s| s.checked_add(bonus_bps))
        .and_then(|s| s.checked_add(nft_reward_bps))
        .and_then(|s| s.checked_add(platform_fee_bps))
        .ok_or(RogueTraderError::MathOverflow)?;
    let expected = deposit_fee_bps.saturating_sub(spread_to_lp_bps);
    require!(split_sum == expected, RogueTraderError::InvalidFeeSplit);

    let clearing_house = &mut ctx.accounts.clearing_house;
    let vault_bump = ctx.bumps.vault;

    // Authority & roles
    clearing_house.authority = ctx.accounts.authority.key();
    clearing_house.settler = ctx.accounts.settler.key();

    // Fee configuration
    clearing_house.deposit_fee_bps = deposit_fee_bps;
    clearing_house.withdrawal_fee_bps = withdrawal_fee_bps;
    clearing_house.referral_bps = referral_bps;
    clearing_house.tier2_referral_bps = tier2_referral_bps;
    clearing_house.bonus_bps = bonus_bps;
    clearing_house.nft_reward_bps = nft_reward_bps;
    clearing_house.platform_fee_bps = platform_fee_bps;

    // Fee recipient wallets
    clearing_house.platform_wallet = ctx.accounts.platform_wallet.key();
    clearing_house.bonus_wallet = ctx.accounts.bonus_wallet.key();
    clearing_house.nft_rewarder = ctx.accounts.nft_rewarder.key();

    // Global state — all zeroed
    clearing_house.paused = false;
    clearing_house.next_bet_id = 0;
    clearing_house.total_bets_proposed = 0;
    clearing_house.total_bets_settled = 0;
    clearing_house.total_volume = 0;
    clearing_house.total_deposit_fees = 0;
    clearing_house.total_withdrawal_fees = 0;
    clearing_house.total_referral_paid = 0;
    clearing_house.total_nft_rewards_paid = 0;
    clearing_house.total_platform_fees_paid = 0;
    clearing_house.total_bonus_paid = 0;

    // Master vault
    clearing_house.vault = ctx.accounts.vault.key();
    clearing_house.vault_bump = vault_bump;

    // ALT — set later via update_config
    clearing_house.vault_lookup_table = Pubkey::default();

    // PDA bump
    clearing_house.bump = ctx.bumps.clearing_house;

    // Odds clamp defaults
    clearing_house.min_odds_bps = 4_500;
    clearing_house.max_odds_bps = 5_500;
    clearing_house.odds_window_size = 10;
    clearing_house.invert_odds = false;

    // Spread: portion of fee that stays in vault for LP holders
    clearing_house.spread_to_lp_bps = spread_to_lp_bps;

    // Counterparty exposure cap: 1% default
    clearing_house.max_cp_exposure_bps = 100;

    // Stale bet buffer: 120 seconds default
    clearing_house.stale_bet_buffer_secs = 120;

    // Reserved
    clearing_house._reserved = [0u8; 110];

    let clock = Clock::get()?;
    emit!(ClearingHouseInitialized {
        authority: clearing_house.authority,
        settler: clearing_house.settler,
        vault: clearing_house.vault,
        deposit_fee_bps,
        withdrawal_fee_bps,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
