use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token};
use crate::state::{AgentVault, ClearingHouseState};
use crate::errors::RogueTraderError;
use crate::events::AgentVaultCreated;

#[derive(Accounts)]
#[instruction(bot_id: u8)]
pub struct CreateAgentVault<'info> {
    #[account(
        mut,
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
        has_one = authority,
    )]
    pub clearing_house: Account<'info, ClearingHouseState>,

    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<AgentVault>(),
        seeds = [b"agent_vault", bot_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub agent_vault: Account<'info, AgentVault>,

    /// LP mint for this bot (9 decimals)
    #[account(
        init,
        payer = authority,
        seeds = [b"bot_lp_mint", bot_id.to_le_bytes().as_ref()],
        bump,
        mint::decimals = 9,
        mint::authority = lp_authority,
    )]
    pub lp_mint: Account<'info, Mint>,

    /// LP authority PDA — signs mint/burn operations
    /// CHECK: PDA validated by seeds
    #[account(
        seeds = [b"bot_lp_authority", bot_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub lp_authority: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(
    ctx: Context<CreateAgentVault>,
    bot_id: u8,
    group_id: u8,
    name: [u8; 16],
) -> Result<()> {
    // Validate bot_id 1-30, group_id 1-5
    require!(bot_id >= 1 && bot_id <= 30, RogueTraderError::InvalidBotId);
    require!(group_id >= 1 && group_id <= 5, RogueTraderError::InvalidGroupId);

    let vault = &mut ctx.accounts.agent_vault;

    // Identity
    vault.bot_id = bot_id;
    vault.group_id = group_id;
    vault.name = name;

    // LP token references
    vault.lp_mint = ctx.accounts.lp_mint.key();
    vault.lp_authority = ctx.accounts.lp_authority.key();
    vault.lp_mint_bump = ctx.bumps.lp_mint;
    vault.lp_authority_bump = ctx.bumps.lp_authority;

    // SOL balance — starts at 0, funded separately via fund_vault
    vault.sol_balance = 0;
    vault.locked_sol = 0;

    // LP tracking
    vault.total_lp_supply = 0;
    vault.total_deposited = 0;
    vault.total_withdrawn = 0;
    vault.deposit_count = 0;
    vault.withdrawal_count = 0;

    // Settler as authorized executor
    vault.authorized_executor = ctx.accounts.clearing_house.settler;

    // Win rate window — all zeros
    vault.bet_window = [0u8; 100];
    vault.window_head = 0;
    vault.window_count = 0;
    vault.wins_in_window = 0;

    // Stats
    vault.bets_proposed = 0;
    vault.bets_won = 0;
    vault.bets_lost = 0;
    vault.bets_tied = 0;

    // Active bets
    vault.active_bet_count = 0;

    // PDA bump
    vault.bump = ctx.bumps.agent_vault;

    // Reserved
    vault.counterparty_locked_sol = 0;
    vault._reserved = [0u8; 56];

    let clock = Clock::get()?;
    emit!(AgentVaultCreated {
        bot_id,
        group_id,
        name,
        lp_mint: vault.lp_mint,
        vault_pubkey: vault.key(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
