use anchor_lang::prelude::*;
use crate::state::ClearingHouseState;
use crate::events::{ProtocolPaused, PauseStateChanged};

#[derive(Accounts)]
pub struct Pause<'info> {
    #[account(
        mut,
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
        has_one = authority,
    )]
    pub clearing_house: Account<'info, ClearingHouseState>,

    pub authority: Signer<'info>,
}

/// L-6: Granular pause — control deposits, withdrawals, and betting independently.
/// The legacy `paused` field is kept in sync (true if ANY operation is paused).
pub fn handler(
    ctx: Context<Pause>,
    paused: bool,
    deposits_paused: Option<bool>,
    withdrawals_paused: Option<bool>,
    betting_paused: Option<bool>,
) -> Result<()> {
    let ch = &mut ctx.accounts.clearing_house;

    // If granular flags are provided, use them; otherwise fall back to the single `paused` flag
    if deposits_paused.is_some() || withdrawals_paused.is_some() || betting_paused.is_some() {
        if let Some(v) = deposits_paused { ch.deposits_paused = v; }
        if let Some(v) = withdrawals_paused { ch.withdrawals_paused = v; }
        if let Some(v) = betting_paused { ch.betting_paused = v; }
    } else {
        // Legacy behavior: single flag controls all operations
        ch.deposits_paused = paused;
        ch.withdrawals_paused = paused;
        ch.betting_paused = paused;
    }

    // Keep legacy `paused` in sync (true if any operation is paused)
    ch.paused = ch.deposits_paused || ch.withdrawals_paused || ch.betting_paused;

    let clock = Clock::get()?;

    // Emit both legacy and granular events
    emit!(ProtocolPaused {
        authority: ctx.accounts.authority.key(),
        paused: ch.paused,
        timestamp: clock.unix_timestamp,
    });
    emit!(PauseStateChanged {
        authority: ctx.accounts.authority.key(),
        deposits_paused: ch.deposits_paused,
        withdrawals_paused: ch.withdrawals_paused,
        betting_paused: ch.betting_paused,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
