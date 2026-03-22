use anchor_lang::prelude::*;
use crate::state::{AgentVault, ClearingHouseState};
use crate::errors::RogueTraderError;

#[derive(Accounts)]
pub struct AdminResetVault<'info> {
    #[account(
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
    )]
    pub clearing_house: Account<'info, ClearingHouseState>,

    #[account(
        mut,
        seeds = [b"agent_vault", agent_vault.bot_id.to_le_bytes().as_ref()],
        bump = agent_vault.bump,
    )]
    pub agent_vault: Account<'info, AgentVault>,

    /// Authority OR settler can reset vault counters
    #[account(
        constraint = signer.key() == clearing_house.authority
            || signer.key() == clearing_house.settler
            @ RogueTraderError::UnauthorizedSettler,
    )]
    pub signer: Signer<'info>,
}

/// Authority-only safety valve: reset a vault's active_bet_count and locked_sol
/// when bet accounts have disappeared without proper cleanup (e.g. Pyth devnet redeploy).
pub fn handler(
    ctx: Context<AdminResetVault>,
    active_bet_count: u8,
    locked_sol: u64,
    counterparty_locked_sol: u64,
) -> Result<()> {
    let vault = &mut ctx.accounts.agent_vault;

    require!(
        locked_sol <= vault.sol_balance,
        RogueTraderError::InvalidConfig
    );
    require!(
        counterparty_locked_sol <= locked_sol,
        RogueTraderError::InvalidConfig
    );

    msg!(
        "AdminResetVault bot={}: active_bet_count {} -> {}, locked_sol {} -> {}, cp_locked {} -> {}",
        vault.bot_id,
        vault.active_bet_count,
        active_bet_count,
        vault.locked_sol,
        locked_sol,
        vault.counterparty_locked_sol,
        counterparty_locked_sol
    );

    vault.active_bet_count = active_bet_count;
    vault.locked_sol = locked_sol;
    vault.counterparty_locked_sol = counterparty_locked_sol;

    Ok(())
}
