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

/// Authority-only safety valve: reset a vault's counters, sol_balance, and LP mint reference.
/// Used for recovery from stuck state or reversing migrations.
pub fn handler(
    ctx: Context<AdminResetVault>,
    active_bet_count: u8,
    locked_sol: u64,
    counterparty_locked_sol: u64,
    sol_balance: Option<u64>,
    lp_mint: Option<Pubkey>,
    lp_mint_bump: Option<u8>,
) -> Result<()> {
    let vault = &mut ctx.accounts.agent_vault;

    // Reset LP mint reference (for reversing token22 migration)
    if let Some(new_lp_mint) = lp_mint {
        msg!(
            "AdminResetVault bot={}: lp_mint {} -> {}, total_lp_supply {} -> 0",
            vault.bot_id,
            vault.lp_mint,
            new_lp_mint,
            vault.total_lp_supply
        );
        vault.lp_mint = new_lp_mint;
        // New mint has zero supply — reset bookkeeping to match
        vault.total_lp_supply = 0;
    }
    if let Some(new_bump) = lp_mint_bump {
        vault.lp_mint_bump = new_bump;
    }

    // Reset sol_balance (for zeroing dust after full withdrawal)
    if let Some(new_sol_balance) = sol_balance {
        msg!(
            "AdminResetVault bot={}: sol_balance {} -> {}",
            vault.bot_id,
            vault.sol_balance,
            new_sol_balance
        );
        vault.sol_balance = new_sol_balance;
    }

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
