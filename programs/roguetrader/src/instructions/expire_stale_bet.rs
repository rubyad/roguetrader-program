use anchor_lang::prelude::*;
use crate::state::{AgentVault, Bet, ClearingHouseState};
use crate::errors::RogueTraderError;
use crate::events::StaleBetExpired;

#[derive(Accounts)]
pub struct ExpireStaleBet<'info> {
    #[account(
        mut,
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
    )]
    pub clearing_house: Box<Account<'info, ClearingHouseState>>,

    /// Proposer bot's AgentVault — must match bet.proposer_bot
    #[account(
        mut,
        seeds = [b"agent_vault", proposer_vault.bot_id.to_le_bytes().as_ref()],
        bump = proposer_vault.bump,
        constraint = proposer_vault.bot_id == bet.proposer_bot @ RogueTraderError::VaultBetMismatch,
    )]
    pub proposer_vault: Box<Account<'info, AgentVault>>,

    /// Bet account to expire — must be past expiry + buffer
    #[account(
        mut,
        seeds = [b"bet", bet.bet_id.to_le_bytes().as_ref()],
        bump = bet.bump,
        close = signer,
    )]
    pub bet: Box<Account<'info, Bet>>,

    /// Authority OR settler can expire stale bets
    #[account(
        mut,
        constraint = signer.key() == clearing_house.authority
            || signer.key() == clearing_house.settler
            @ RogueTraderError::UnauthorizedSettler,
    )]
    pub signer: Signer<'info>,

    // 29 counterparty AgentVault accounts passed as remaining_accounts
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, ExpireStaleBet<'info>>,
) -> Result<()> {
    let bet = &ctx.accounts.bet;
    let clock = Clock::get()?;

    // Guards
    require!(!bet.settled, RogueTraderError::BetAlreadySettled);
    let buffer = if ctx.accounts.clearing_house.stale_bet_buffer_secs > 0 {
        ctx.accounts.clearing_house.stale_bet_buffer_secs
    } else {
        120 // default
    };
    require!(
        clock.unix_timestamp >= bet.expiry_timestamp + buffer,
        RogueTraderError::StaleBetBufferNotElapsed
    );

    let proposer_stake = bet.proposer_stake;

    // M-4: Validate counterparty vaults — PDA check, uniqueness, completeness
    let mut seen_bot_ids: [bool; 31] = [false; 31];
    for acct in ctx.remaining_accounts.iter() {
        let cp_vault_check = Account::<AgentVault>::try_from(acct)?;
        let (expected_pda, _) = Pubkey::find_program_address(
            &[b"agent_vault", cp_vault_check.bot_id.to_le_bytes().as_ref()],
            &crate::ID,
        );
        require!(acct.key() == expected_pda, RogueTraderError::InvalidCounterpartyVault);
        let bid = cp_vault_check.bot_id as usize;
        require!(bid <= 30, RogueTraderError::InvalidCounterpartyVault);
        require!(!seen_bot_ids[bid], RogueTraderError::DuplicateCounterparty);
        seen_bot_ids[bid] = true;
    }
    for cp in bet.counterparties[..bet.cp_count as usize].iter() {
        if cp.stake > 0 {
            require!(
                seen_bot_ids[cp.bot_id as usize],
                RogueTraderError::MissingCounterparty
            );
        }
    }

    // Unlock all counterparty capital (no winner/loser, pure unlock)
    let mut total_unlocked: u64 = 0;
    for acct in ctx.remaining_accounts.iter() {
        let mut cp_vault = Account::<AgentVault>::try_from(acct)?;

        let cp_stake = bet.counterparties[..bet.cp_count as usize]
            .iter()
            .find(|cp| cp.bot_id == cp_vault.bot_id)
            .map(|cp| cp.stake)
            .unwrap_or(0);

        if cp_stake > 0 {
            cp_vault.locked_sol = cp_vault.locked_sol.saturating_sub(cp_stake);
            cp_vault.counterparty_locked_sol = cp_vault.counterparty_locked_sol.saturating_sub(cp_stake);
            total_unlocked += cp_stake;
        }

        cp_vault.exit(&crate::ID)?;
    }

    // Unlock proposer's capital
    let proposer = &mut ctx.accounts.proposer_vault;
    proposer.locked_sol = proposer.locked_sol.saturating_sub(proposer_stake);
    proposer.active_bet_count = proposer.active_bet_count.saturating_sub(1);
    // No win rate update for expired bets (same as tie)

    emit!(StaleBetExpired {
        bet_id: bet.bet_id,
        proposer_bot: proposer.bot_id,
        locked_sol_returned: proposer_stake + total_unlocked,
        timestamp: clock.unix_timestamp,
    });

    // Anchor's `close = signer` handles rent reclaim
    Ok(())
}
