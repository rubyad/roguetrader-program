use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::state::{AgentVault, ClearingHouseState};
use crate::errors::RogueTraderError;
use crate::events::VaultFunded;

#[derive(Accounts)]
#[instruction(bot_id: u8)]
pub struct FundVault<'info> {
    #[account(
        mut,
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
        has_one = authority,
    )]
    pub clearing_house: Account<'info, ClearingHouseState>,

    #[account(
        mut,
        seeds = [b"agent_vault", bot_id.to_le_bytes().as_ref()],
        bump = agent_vault.bump,
    )]
    pub agent_vault: Account<'info, AgentVault>,

    /// Master vault PDA
    #[account(
        mut,
        seeds = [b"vault"],
        bump = clearing_house.vault_bump,
    )]
    /// CHECK: System-owned PDA, validated by seeds
    pub vault: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<FundVault>,
    _bot_id: u8,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, RogueTraderError::ZeroAmount);

    // Transfer SOL from authority to master vault PDA
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.authority.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
            },
        ),
        amount,
    )?;

    // Increment bot's sol_balance (bookkeeping)
    let vault = &mut ctx.accounts.agent_vault;
    vault.sol_balance = vault
        .sol_balance
        .checked_add(amount)
        .ok_or(RogueTraderError::MathOverflow)?;

    let clock = Clock::get()?;
    emit!(VaultFunded {
        bot_id: vault.bot_id,
        amount,
        new_balance: vault.sol_balance,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
