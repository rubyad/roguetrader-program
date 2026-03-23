use anchor_lang::prelude::*;
use crate::state::ClearingHouseState;
use crate::errors::RogueTraderError;
use crate::events::AuthorityTransferred;

// ============================================================================
// Propose Authority Transfer (current authority proposes a new authority)
// ============================================================================

#[derive(Accounts)]
pub struct ProposeAuthorityTransfer<'info> {
    #[account(
        mut,
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
        has_one = authority,
    )]
    pub clearing_house: Account<'info, ClearingHouseState>,

    pub authority: Signer<'info>,

    /// CHECK: New authority — can be any valid pubkey (including multi-sig)
    pub new_authority: AccountInfo<'info>,
}

pub fn propose_handler(ctx: Context<ProposeAuthorityTransfer>) -> Result<()> {
    ctx.accounts.clearing_house.pending_authority = ctx.accounts.new_authority.key();
    Ok(())
}

// ============================================================================
// Accept Authority Transfer (new authority signs to accept)
// ============================================================================

#[derive(Accounts)]
pub struct AcceptAuthorityTransfer<'info> {
    #[account(
        mut,
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
    )]
    pub clearing_house: Account<'info, ClearingHouseState>,

    #[account(
        constraint = new_authority.key() == clearing_house.pending_authority
            @ RogueTraderError::InvalidPendingAuthority,
    )]
    pub new_authority: Signer<'info>,
}

pub fn accept_handler(ctx: Context<AcceptAuthorityTransfer>) -> Result<()> {
    require!(
        ctx.accounts.clearing_house.pending_authority != Pubkey::default(),
        RogueTraderError::NoPendingTransfer
    );

    let old = ctx.accounts.clearing_house.authority;
    ctx.accounts.clearing_house.authority = ctx.accounts.clearing_house.pending_authority;
    ctx.accounts.clearing_house.pending_authority = Pubkey::default();

    let clock = Clock::get()?;
    emit!(AuthorityTransferred {
        old_authority: old,
        new_authority: ctx.accounts.clearing_house.authority,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
