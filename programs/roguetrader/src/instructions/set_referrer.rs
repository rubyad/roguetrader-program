use anchor_lang::prelude::*;
use crate::state::{ClearingHouseState, PlayerState, ReferralState};
use crate::errors::RogueTraderError;
use crate::events::ReferrerSet;

#[derive(Accounts)]
pub struct SetReferrer<'info> {
    #[account(
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
    )]
    pub clearing_house: Account<'info, ClearingHouseState>,

    #[account(
        mut,
        seeds = [b"player_state", player.key().as_ref()],
        bump = player_state.bump,
    )]
    pub player_state: Account<'info, PlayerState>,

    /// Referrer's player state (for tier-2 resolution)
    #[account(
        seeds = [b"player_state", referrer.key().as_ref()],
        bump = referrer_player_state.bump,
    )]
    pub referrer_player_state: Account<'info, PlayerState>,

    /// Referrer's referral state
    #[account(
        mut,
        seeds = [b"referral_state", referrer.key().as_ref()],
        bump = referral_state.bump,
    )]
    pub referral_state: Account<'info, ReferralState>,

    pub player: Signer<'info>,

    /// CHECK: Referrer wallet
    pub referrer: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<SetReferrer>) -> Result<()> {
    let player_state = &mut ctx.accounts.player_state;
    let referrer_key = ctx.accounts.referrer.key();

    // Guards
    require!(referrer_key != ctx.accounts.player.key(), RogueTraderError::SelfReferral);
    require!(player_state.referrer == Pubkey::default(), RogueTraderError::ReferrerAlreadySet);
    require!(referrer_key != Pubkey::default(), RogueTraderError::InvalidReferrer);

    // Set referrer
    player_state.referrer = referrer_key;

    // Auto-resolve tier-2: referrer's referrer becomes tier-2
    let referrer_ps = &ctx.accounts.referrer_player_state;
    if referrer_ps.referrer != Pubkey::default() {
        player_state.tier2_referrer = referrer_ps.referrer;
    }

    // Increment referral count
    let referral_state = &mut ctx.accounts.referral_state;
    referral_state.referral_count += 1;

    let clock = Clock::get()?;
    emit!(ReferrerSet {
        player: ctx.accounts.player.key(),
        referrer: referrer_key,
        tier2_referrer: player_state.tier2_referrer,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
