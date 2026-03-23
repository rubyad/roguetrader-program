use anchor_lang::prelude::*;
use crate::state::{Bet, ClearingHouseState};
use crate::events::BetClosed;

#[derive(Accounts)]
pub struct CloseBet<'info> {
    #[account(
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
    )]
    pub clearing_house: Account<'info, ClearingHouseState>,

    /// Bet account to close — must be settled.
    /// Anchor's `close` attribute handles the lamport transfer and zeroing.
    #[account(
        mut,
        seeds = [b"bet", bet.bet_id.to_le_bytes().as_ref()],
        bump = bet.bump,
        constraint = bet.settled @ crate::errors::RogueTraderError::BetNotSettled,
        close = rent_receiver,
    )]
    pub bet: Account<'info, Bet>,

    /// Receives the rent from closing the bet account — must be authority or settler
    #[account(
        mut,
        constraint = rent_receiver.key() == clearing_house.authority
            || rent_receiver.key() == clearing_house.settler
            @ crate::errors::RogueTraderError::Unauthorized
    )]
    pub rent_receiver: Signer<'info>,
}

pub fn handler(ctx: Context<CloseBet>) -> Result<()> {
    let clock = Clock::get()?;
    emit!(BetClosed {
        bet_id: ctx.accounts.bet.bet_id,
        rent_returned_to: ctx.accounts.rent_receiver.key(),
        timestamp: clock.unix_timestamp,
    });
    // Anchor close constraint handles the rest
    Ok(())
}
