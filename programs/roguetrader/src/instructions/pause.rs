use anchor_lang::prelude::*;
use crate::state::ClearingHouseState;
use crate::events::ProtocolPaused;

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

pub fn handler(ctx: Context<Pause>, paused: bool) -> Result<()> {
    ctx.accounts.clearing_house.paused = paused;

    let clock = Clock::get()?;
    emit!(ProtocolPaused {
        authority: ctx.accounts.authority.key(),
        paused,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
