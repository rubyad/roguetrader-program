use anchor_lang::prelude::*;
use crate::state::ClearingHouseState;
use crate::errors::RogueTraderError;
use crate::events::ConfigUpdated;

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(
        mut,
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
        has_one = authority,
    )]
    pub clearing_house: Account<'info, ClearingHouseState>,

    pub authority: Signer<'info>,
}

#[allow(clippy::too_many_arguments)]
pub fn handler(
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
    let ch = &mut ctx.accounts.clearing_house;
    let clock = Clock::get()?;

    if let Some(v) = deposit_fee_bps {
        require!(v <= 1000, RogueTraderError::InvalidConfig);
        let old = ch.deposit_fee_bps;
        ch.deposit_fee_bps = v;
        emit!(ConfigUpdated { field_id: 0, old_value: old as u64, new_value: v as u64, authority: ctx.accounts.authority.key(), timestamp: clock.unix_timestamp });
    }
    if let Some(v) = withdrawal_fee_bps {
        require!(v <= 1000, RogueTraderError::InvalidConfig);
        let old = ch.withdrawal_fee_bps;
        ch.withdrawal_fee_bps = v;
        emit!(ConfigUpdated { field_id: 1, old_value: old as u64, new_value: v as u64, authority: ctx.accounts.authority.key(), timestamp: clock.unix_timestamp });
    }
    if let Some(v) = referral_bps { ch.referral_bps = v; }
    if let Some(v) = tier2_referral_bps { ch.tier2_referral_bps = v; }
    if let Some(v) = bonus_bps { ch.bonus_bps = v; }
    if let Some(v) = nft_reward_bps { ch.nft_reward_bps = v; }
    if let Some(v) = platform_fee_bps { ch.platform_fee_bps = v; }
    if let Some(v) = platform_wallet { ch.platform_wallet = v; }
    if let Some(v) = bonus_wallet { ch.bonus_wallet = v; }
    if let Some(v) = nft_rewarder { ch.nft_rewarder = v; }
    if let Some(v) = settler { ch.settler = v; }
    if let Some(v) = vault_lookup_table { ch.vault_lookup_table = v; }
    if let Some(v) = min_odds_bps {
        require!(v <= 10_000, RogueTraderError::InvalidConfig);
        ch.min_odds_bps = v;
    }
    if let Some(v) = max_odds_bps {
        require!(v <= 10_000, RogueTraderError::InvalidConfig);
        ch.max_odds_bps = v;
    }
    if let Some(v) = odds_window_size {
        require!(v >= 1 && v <= 100, RogueTraderError::InvalidConfig);
        ch.odds_window_size = v;
    }
    if let Some(v) = invert_odds {
        ch.invert_odds = v;
    }
    if let Some(v) = spread_to_lp_bps {
        require!(v <= ch.deposit_fee_bps, RogueTraderError::InvalidConfig);
        ch.spread_to_lp_bps = v;
    }
    if let Some(v) = max_cp_exposure_bps {
        require!(v <= 10_000, RogueTraderError::InvalidConfig);
        ch.max_cp_exposure_bps = v;
    }
    if let Some(v) = stale_bet_buffer_secs {
        require!(v >= 0 && v <= 600, RogueTraderError::InvalidConfig);
        ch.stale_bet_buffer_secs = v;
    }

    // Validate fee split: wallet splits must sum to deposit_fee_bps minus spread_to_lp_bps
    let split_sum = ch.referral_bps
        .checked_add(ch.tier2_referral_bps)
        .and_then(|s| s.checked_add(ch.bonus_bps))
        .and_then(|s| s.checked_add(ch.nft_reward_bps))
        .and_then(|s| s.checked_add(ch.platform_fee_bps))
        .ok_or(RogueTraderError::MathOverflow)?;
    let expected = ch.deposit_fee_bps.saturating_sub(ch.spread_to_lp_bps);
    require!(split_sum == expected, RogueTraderError::InvalidFeeSplit);

    Ok(())
}
