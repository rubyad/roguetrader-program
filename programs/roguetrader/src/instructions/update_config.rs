use anchor_lang::prelude::*;
use crate::state::ClearingHouseState;
use crate::errors::RogueTraderError;
use crate::events::{ConfigUpdated, ConfigPubkeyUpdated};

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
    // M-9: Emit events for ALL config changes
    let auth = ctx.accounts.authority.key();
    let ts = clock.unix_timestamp;

    if let Some(v) = referral_bps {
        let old = ch.referral_bps;
        ch.referral_bps = v;
        emit!(ConfigUpdated { field_id: 2, old_value: old as u64, new_value: v as u64, authority: auth, timestamp: ts });
    }
    if let Some(v) = tier2_referral_bps {
        let old = ch.tier2_referral_bps;
        ch.tier2_referral_bps = v;
        emit!(ConfigUpdated { field_id: 3, old_value: old as u64, new_value: v as u64, authority: auth, timestamp: ts });
    }
    if let Some(v) = bonus_bps {
        let old = ch.bonus_bps;
        ch.bonus_bps = v;
        emit!(ConfigUpdated { field_id: 4, old_value: old as u64, new_value: v as u64, authority: auth, timestamp: ts });
    }
    if let Some(v) = nft_reward_bps {
        let old = ch.nft_reward_bps;
        ch.nft_reward_bps = v;
        emit!(ConfigUpdated { field_id: 5, old_value: old as u64, new_value: v as u64, authority: auth, timestamp: ts });
    }
    if let Some(v) = platform_fee_bps {
        let old = ch.platform_fee_bps;
        ch.platform_fee_bps = v;
        emit!(ConfigUpdated { field_id: 6, old_value: old as u64, new_value: v as u64, authority: auth, timestamp: ts });
    }
    if let Some(v) = platform_wallet {
        let old = ch.platform_wallet;
        ch.platform_wallet = v;
        emit!(ConfigPubkeyUpdated { field_id: 7, old_value: old, new_value: v, authority: auth, timestamp: ts });
    }
    if let Some(v) = bonus_wallet {
        let old = ch.bonus_wallet;
        ch.bonus_wallet = v;
        emit!(ConfigPubkeyUpdated { field_id: 8, old_value: old, new_value: v, authority: auth, timestamp: ts });
    }
    if let Some(v) = nft_rewarder {
        let old = ch.nft_rewarder;
        ch.nft_rewarder = v;
        emit!(ConfigPubkeyUpdated { field_id: 9, old_value: old, new_value: v, authority: auth, timestamp: ts });
    }
    if let Some(v) = settler {
        let old = ch.settler;
        ch.settler = v;
        emit!(ConfigPubkeyUpdated { field_id: 10, old_value: old, new_value: v, authority: auth, timestamp: ts });
    }
    if let Some(v) = vault_lookup_table {
        let old = ch.vault_lookup_table;
        ch.vault_lookup_table = v;
        emit!(ConfigPubkeyUpdated { field_id: 11, old_value: old, new_value: v, authority: auth, timestamp: ts });
    }
    if let Some(v) = min_odds_bps {
        require!(v <= 10_000, RogueTraderError::InvalidConfig);
        let old = ch.min_odds_bps;
        ch.min_odds_bps = v;
        emit!(ConfigUpdated { field_id: 12, old_value: old as u64, new_value: v as u64, authority: auth, timestamp: ts });
    }
    if let Some(v) = max_odds_bps {
        require!(v <= 10_000, RogueTraderError::InvalidConfig);
        let old = ch.max_odds_bps;
        ch.max_odds_bps = v;
        emit!(ConfigUpdated { field_id: 13, old_value: old as u64, new_value: v as u64, authority: auth, timestamp: ts });
    }
    if let Some(v) = odds_window_size {
        require!(v >= 1 && v <= 100, RogueTraderError::InvalidConfig);
        let old = ch.odds_window_size;
        ch.odds_window_size = v;
        emit!(ConfigUpdated { field_id: 14, old_value: old as u64, new_value: v as u64, authority: auth, timestamp: ts });
    }
    if let Some(v) = invert_odds {
        let old = ch.invert_odds;
        ch.invert_odds = v;
        emit!(ConfigUpdated { field_id: 15, old_value: old as u64, new_value: v as u64, authority: auth, timestamp: ts });
    }
    if let Some(v) = spread_to_lp_bps {
        require!(v <= ch.deposit_fee_bps, RogueTraderError::InvalidConfig);
        let old = ch.spread_to_lp_bps;
        ch.spread_to_lp_bps = v;
        emit!(ConfigUpdated { field_id: 16, old_value: old as u64, new_value: v as u64, authority: auth, timestamp: ts });
    }
    if let Some(v) = max_cp_exposure_bps {
        require!(v <= 10_000, RogueTraderError::InvalidConfig);
        let old = ch.max_cp_exposure_bps;
        ch.max_cp_exposure_bps = v;
        emit!(ConfigUpdated { field_id: 17, old_value: old as u64, new_value: v as u64, authority: auth, timestamp: ts });
    }
    if let Some(v) = stale_bet_buffer_secs {
        require!(v >= 0 && v <= 600, RogueTraderError::InvalidConfig);
        let old = ch.stale_bet_buffer_secs;
        ch.stale_bet_buffer_secs = v;
        emit!(ConfigUpdated { field_id: 18, old_value: old as u64, new_value: v as u64, authority: auth, timestamp: ts });
    }

    // Validate fee split: wallet splits must sum to deposit_fee_bps minus spread_to_lp_bps
    let split_sum = ch.referral_bps
        .checked_add(ch.tier2_referral_bps)
        .and_then(|s| s.checked_add(ch.bonus_bps))
        .and_then(|s| s.checked_add(ch.nft_reward_bps))
        .and_then(|s| s.checked_add(ch.platform_fee_bps))
        .ok_or(RogueTraderError::MathOverflow)?;
    let deposit_wallet_bps = ch.deposit_fee_bps.saturating_sub(ch.spread_to_lp_bps);
    require!(split_sum == deposit_wallet_bps, RogueTraderError::InvalidFeeSplit);

    // M-2/M-8: Also validate against withdrawal fee (same splits must fit)
    let withdrawal_wallet_bps = ch.withdrawal_fee_bps.saturating_sub(ch.spread_to_lp_bps);
    require!(split_sum == withdrawal_wallet_bps, RogueTraderError::InvalidWithdrawalFeeSplit);

    Ok(())
}
