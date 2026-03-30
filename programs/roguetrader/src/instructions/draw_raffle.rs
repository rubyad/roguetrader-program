use anchor_lang::prelude::*;
use crate::state::{AgentVault, ClearingHouseState};
use crate::errors::RogueTraderError;
use crate::events::RaffleDrawn;

#[derive(Accounts)]
pub struct DrawRaffle<'info> {
    #[account(
        mut,
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
    )]
    pub clearing_house: Box<Account<'info, ClearingHouseState>>,

    /// Settler signer — only settler can trigger raffle
    #[account(
        constraint = settler.key() == clearing_house.settler @ RogueTraderError::UnauthorizedSettler,
    )]
    pub settler: Signer<'info>,

    // All 30 AgentVault accounts passed as remaining_accounts
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, DrawRaffle<'info>>,
) -> Result<()> {
    let ch = &ctx.accounts.clearing_house;
    let clock = Clock::get()?;

    // Guards
    require!(!ch.raffle_paused, RogueTraderError::RafflePaused);
    require!(ch.rewards_pool_balance > 0, RogueTraderError::EmptyRewardsPool);

    // Check interval elapsed (last_raffle_timestamp == 0 means never drawn, allow immediately)
    if ch.last_raffle_timestamp > 0 && ch.raffle_interval_secs > 0 {
        let next_raffle_time = ch.last_raffle_timestamp
            .checked_add(ch.raffle_interval_secs)
            .ok_or(RogueTraderError::MathOverflow)?;
        require!(
            clock.unix_timestamp >= next_raffle_time,
            RogueTraderError::RaffleTooEarly
        );
    }

    let reward_amount = ch.rewards_pool_balance;
    let raffle_number = ch.total_raffles_drawn;

    // First pass: validate PDAs and collect (bot_id, sol_balance) for all 30 vaults
    let mut bot_balances: [(u8, u64); 30] = [(0, 0); 30];
    let mut vault_count: usize = 0;

    for acct in ctx.remaining_accounts.iter() {
        let vault = Account::<AgentVault>::try_from(acct)?;
        let (expected_pda, _) = Pubkey::find_program_address(
            &[b"agent_vault", vault.bot_id.to_le_bytes().as_ref()],
            ctx.program_id,
        );
        require!(acct.key() == expected_pda, RogueTraderError::InvalidCounterpartyVault);
        require!(vault_count < 30, RogueTraderError::InvalidBotId);
        bot_balances[vault_count] = (vault.bot_id, vault.sol_balance);
        vault_count += 1;
    }

    // Compute inverse-AUM weights: weight_i = max_aum - aum_i + 1
    let max_aum = bot_balances[..vault_count].iter().map(|(_, b)| *b).max().unwrap_or(0);
    let mut total_weight: u128 = 0;
    let mut weights: [u128; 30] = [0; 30];
    for i in 0..vault_count {
        let w = (max_aum as u128) - (bot_balances[i].1 as u128) + 1;
        weights[i] = w;
        total_weight += w;
    }

    // Provable random seed: blake3(slot || total_raffles_drawn)
    let slot_bytes = clock.slot.to_le_bytes();
    let nonce_bytes = raffle_number.to_le_bytes();
    let mut hash_input = [0u8; 16];
    hash_input[..8].copy_from_slice(&slot_bytes);
    hash_input[8..16].copy_from_slice(&nonce_bytes);
    let hash = blake3::hash(&hash_input);
    let hash_bytes = hash.as_bytes();
    let random_value = u64::from_le_bytes([
        hash_bytes[0], hash_bytes[1], hash_bytes[2], hash_bytes[3],
        hash_bytes[4], hash_bytes[5], hash_bytes[6], hash_bytes[7],
    ]);

    // Weighted selection
    let target = (random_value as u128) % total_weight;
    let mut cumulative: u128 = 0;
    let mut winner_idx: usize = 0;
    for i in 0..vault_count {
        cumulative += weights[i];
        if target < cumulative {
            winner_idx = i;
            break;
        }
    }
    let winner_bot_id = bot_balances[winner_idx].0;

    // Second pass: find winner vault and add reward
    for acct in ctx.remaining_accounts.iter() {
        let mut vault = Account::<AgentVault>::try_from(acct)?;
        if vault.bot_id == winner_bot_id {
            vault.sol_balance = vault.sol_balance
                .checked_add(reward_amount)
                .ok_or(RogueTraderError::MathOverflow)?;
            vault.exit(&crate::ID)?;
            break;
        }
    }

    // Update ClearingHouseState
    let ch = &mut ctx.accounts.clearing_house;
    ch.rewards_pool_balance = 0;
    ch.last_raffle_timestamp = clock.unix_timestamp;
    ch.total_raffles_drawn = raffle_number.checked_add(1).ok_or(RogueTraderError::MathOverflow)?;
    ch.total_rewards_distributed = ch.total_rewards_distributed
        .checked_add(reward_amount)
        .ok_or(RogueTraderError::MathOverflow)?;
    ch.last_winner_bot_id = winner_bot_id;
    ch.last_winner_amount = reward_amount;

    emit!(RaffleDrawn {
        raffle_number,
        winner_bot_id,
        reward_amount,
        slot: clock.slot,
        timestamp: clock.unix_timestamp,
        total_weight: total_weight as u64,
    });

    Ok(())
}
