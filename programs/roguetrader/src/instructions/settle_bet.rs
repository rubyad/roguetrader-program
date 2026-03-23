use anchor_lang::prelude::*;
use crate::state::{AgentVault, Bet, ClearingHouseState};
use crate::state::bet::{OUTCOME_PROPOSER_WON, OUTCOME_PROPOSER_LOST, OUTCOME_TIE};
use crate::errors::RogueTraderError;
use crate::events::BetSettled;
use crate::pyth::PriceUpdateV2;

const MAX_PRICE_AGE: u64 = 60;
const MAX_CONF_BPS: u64 = 200;

#[derive(Accounts)]
pub struct SettleBet<'info> {
    #[account(
        mut,
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
    )]
    pub clearing_house: Box<Account<'info, ClearingHouseState>>,

    /// Proposer bot's AgentVault (for win rate update) — must match bet.proposer_bot
    #[account(
        mut,
        seeds = [b"agent_vault", proposer_vault.bot_id.to_le_bytes().as_ref()],
        bump = proposer_vault.bump,
        constraint = proposer_vault.bot_id == bet.proposer_bot @ RogueTraderError::VaultBetMismatch,
    )]
    pub proposer_vault: Box<Account<'info, AgentVault>>,

    /// Bet account to settle
    #[account(
        mut,
        seeds = [b"bet", bet.bet_id.to_le_bytes().as_ref()],
        bump = bet.bump,
    )]
    pub bet: Box<Account<'info, Bet>>,

    /// Pyth price feed — must match bet.pyth_feed and be owned by Pyth Receiver
    /// CHECK: Owner validated here; data validated in handler
    #[account(
        owner = crate::pyth::PYTH_RECEIVER_PROGRAM_ID @ RogueTraderError::InvalidPythAccount
    )]
    pub pyth_price_feed: AccountInfo<'info>,

    /// Settler signer
    #[account(
        constraint = settler.key() == clearing_house.settler @ RogueTraderError::UnauthorizedSettler,
    )]
    pub settler: Signer<'info>,

    // 29 counterparty AgentVault accounts passed as remaining_accounts
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, SettleBet<'info>>,
) -> Result<()> {
    let bet = &ctx.accounts.bet;

    // Guards
    require!(!bet.settled, RogueTraderError::BetAlreadySettled);
    let clock = Clock::get()?;
    require!(clock.unix_timestamp >= bet.expiry_timestamp, RogueTraderError::BetNotExpired);

    // M-3: Settlement must occur within the stale_bet_buffer window
    let buffer = if ctx.accounts.clearing_house.stale_bet_buffer_secs > 0 {
        ctx.accounts.clearing_house.stale_bet_buffer_secs
    } else {
        120 // default
    };
    let max_settle_time = bet.expiry_timestamp
        .checked_add(buffer)
        .ok_or(RogueTraderError::MathOverflow)?;
    require!(
        clock.unix_timestamp <= max_settle_time,
        RogueTraderError::SettlementWindowExpired
    );

    // Validate Pyth feed matches bet
    require!(
        ctx.accounts.pyth_price_feed.key() == bet.pyth_feed,
        RogueTraderError::FeedMismatch
    );

    // Read exit price
    let (exit_price, exit_conf, exit_expo) = {
        let price_data = ctx.accounts.pyth_price_feed.try_borrow_data()?;
        let price_update = PriceUpdateV2::try_deserialize(&price_data)?;
        let price_msg = price_update
            .get_price_no_older_than(&clock, MAX_PRICE_AGE)?;

        // M-5: Reject non-positive prices
        require!(price_msg.price > 0, RogueTraderError::InvalidPrice);
        let abs_price = price_msg.price as u64; // Safe after positive check

        // Validate confidence
        let conf_bps = (price_msg.conf as u128)
            .checked_mul(10_000)
            .unwrap()
            / (abs_price as u128);
        require!(conf_bps <= MAX_CONF_BPS as u128, RogueTraderError::PythConfidenceTooWide);

        // Validate exponent hasn't changed
        require!(price_msg.exponent == bet.entry_expo, RogueTraderError::ExponentChanged);

        (price_msg.price, price_msg.conf, price_msg.exponent)
    };

    // Determine outcome
    let entry = bet.entry_price;
    let exit = exit_price;
    let proposer_stake = bet.proposer_stake;
    let cp_pool = bet.counterparty_pool;
    let bet_id = bet.bet_id;

    let outcome = match bet.direction {
        crate::state::bet::Direction::Long => {
            if exit > entry { OUTCOME_PROPOSER_WON }
            else if exit < entry { OUTCOME_PROPOSER_LOST }
            else { OUTCOME_TIE }
        }
        crate::state::bet::Direction::Short => {
            if exit < entry { OUTCOME_PROPOSER_WON }
            else if exit > entry { OUTCOME_PROPOSER_LOST }
            else { OUTCOME_TIE }
        }
    };

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

    // Verify all counterparties from the bet are present
    for cp in bet.counterparties[..bet.cp_count as usize].iter() {
        if cp.stake > 0 {
            require!(
                seen_bot_ids[cp.bot_id as usize],
                RogueTraderError::MissingCounterparty
            );
        }
    }

    // Reset for actual processing
    // Redistribute sol_balance across counterparties
    for acct in ctx.remaining_accounts.iter() {
        let mut cp_vault = Account::<AgentVault>::try_from(acct)?;

        // Find this cp's stake in the bet
        let cp_stake = bet.counterparties[..bet.cp_count as usize]
            .iter()
            .find(|cp| cp.bot_id == cp_vault.bot_id)
            .map(|cp| cp.stake)
            .unwrap_or(0);

        if cp_stake == 0 {
            continue;
        }

        // Unlock capital
        msg!("settle cp bot={} stake={} locked={} cp_locked={}", cp_vault.bot_id, cp_stake, cp_vault.locked_sol, cp_vault.counterparty_locked_sol);
        cp_vault.locked_sol = cp_vault.locked_sol.saturating_sub(cp_stake);
        cp_vault.counterparty_locked_sol = cp_vault.counterparty_locked_sol.saturating_sub(cp_stake);

        match outcome {
            OUTCOME_PROPOSER_WON => {
                // Counterparty loses: sol_balance -= cp_stake
                cp_vault.sol_balance = cp_vault.sol_balance.saturating_sub(cp_stake);
            }
            OUTCOME_PROPOSER_LOST => {
                // Counterparty wins proportionally: gains (cp_stake / cp_pool) × proposer_stake
                if cp_pool > 0 {
                    let gain = (cp_stake as u128)
                        .checked_mul(proposer_stake as u128)
                        .unwrap()
                        / (cp_pool as u128);
                    cp_vault.sol_balance = cp_vault
                        .sol_balance
                        .checked_add(gain as u64)
                        .ok_or(RogueTraderError::MathOverflow)?;
                }
            }
            OUTCOME_TIE => {
                // No balance changes, just unlock
            }
            _ => {}
        }

        cp_vault.exit(&crate::ID)?;
    }

    // Update proposer vault
    let proposer = &mut ctx.accounts.proposer_vault;
    proposer.locked_sol = proposer.locked_sol.saturating_sub(proposer_stake);
    proposer.active_bet_count = proposer.active_bet_count.saturating_sub(1);

    match outcome {
        OUTCOME_PROPOSER_WON => {
            proposer.sol_balance = proposer
                .sol_balance
                .checked_add(cp_pool)
                .ok_or(RogueTraderError::MathOverflow)?;
            proposer.bets_won += 1;
            proposer.update_win_rate(true, ctx.accounts.clearing_house.odds_window_size);
        }
        OUTCOME_PROPOSER_LOST => {
            proposer.sol_balance = proposer.sol_balance.saturating_sub(proposer_stake);
            proposer.bets_lost += 1;
            proposer.update_win_rate(false, ctx.accounts.clearing_house.odds_window_size);
        }
        OUTCOME_TIE => {
            proposer.bets_tied += 1;
            // No win rate update for ties
        }
        _ => {}
    }

    // Update bet account
    let bet = &mut ctx.accounts.bet;
    bet.settled = true;
    bet.outcome = outcome;
    bet.exit_price = exit_price;
    bet.exit_conf = exit_conf;
    bet.settle_timestamp = clock.unix_timestamp;

    // Update global stats
    let ch = &mut ctx.accounts.clearing_house;
    ch.total_bets_settled = ch
        .total_bets_settled
        .checked_add(1)
        .ok_or(RogueTraderError::MathOverflow)?;

    emit!(BetSettled {
        bet_id,
        proposer_bot: proposer.bot_id,
        outcome,
        entry_price: entry,
        exit_price,
        proposer_stake,
        counterparty_pool: cp_pool,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
