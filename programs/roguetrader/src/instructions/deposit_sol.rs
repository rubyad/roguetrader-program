use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};
use crate::state::{AgentVault, ClearingHouseState, PlayerState};
use crate::errors::RogueTraderError;
use crate::events::{DepositCompleted, FeeTransferFailed};

#[derive(Accounts)]
pub struct DepositSol<'info> {
    #[account(
        mut,
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
    )]
    pub clearing_house: Box<Account<'info, ClearingHouseState>>,

    #[account(
        mut,
        seeds = [b"agent_vault", agent_vault.bot_id.to_le_bytes().as_ref()],
        bump = agent_vault.bump,
    )]
    pub agent_vault: Box<Account<'info, AgentVault>>,

    /// Master vault PDA
    /// CHECK: System-owned PDA, validated by seeds
    #[account(
        mut,
        seeds = [b"vault"],
        bump = clearing_house.vault_bump,
    )]
    pub vault: AccountInfo<'info>,

    /// LP mint for this bot
    #[account(
        mut,
        address = agent_vault.lp_mint,
    )]
    pub lp_mint: Account<'info, Mint>,

    /// LP authority PDA — signs mint operations
    /// CHECK: PDA validated by seeds
    #[account(
        seeds = [b"bot_lp_authority", agent_vault.bot_id.to_le_bytes().as_ref()],
        bump = agent_vault.lp_authority_bump,
    )]
    pub lp_authority: AccountInfo<'info>,

    /// User's LP token account (ATA) — I-6: validated mint and owner at Anchor level
    #[account(
        mut,
        token::mint = lp_mint,
        token::authority = depositor,
    )]
    pub user_lp_account: Account<'info, TokenAccount>,

    /// Player state for referral tracking (auto-created on first deposit)
    #[account(
        init_if_needed,
        payer = depositor,
        space = 8 + 193,
        seeds = [b"player_state", depositor.key().as_ref()],
        bump,
    )]
    pub player_state: Account<'info, PlayerState>,

    #[account(mut)]
    pub depositor: Signer<'info>,

    /// CHECK: Tier-1 referrer wallet — validated against PlayerState
    #[account(
        mut,
        constraint = referrer.key() == player_state.referrer @ RogueTraderError::InvalidFeeWallet
    )]
    pub referrer: AccountInfo<'info>,

    /// CHECK: Tier-2 referrer wallet — validated against PlayerState
    #[account(
        mut,
        constraint = tier2_referrer.key() == player_state.tier2_referrer @ RogueTraderError::InvalidFeeWallet
    )]
    pub tier2_referrer: AccountInfo<'info>,

    /// CHECK: Bonus wallet — validated against ClearingHouseState
    #[account(
        mut,
        constraint = bonus_wallet.key() == clearing_house.bonus_wallet @ RogueTraderError::InvalidFeeWallet
    )]
    pub bonus_wallet: AccountInfo<'info>,

    /// CHECK: NFT rewarder wallet — validated against ClearingHouseState
    #[account(
        mut,
        constraint = nft_rewarder.key() == clearing_house.nft_rewarder @ RogueTraderError::InvalidFeeWallet
    )]
    pub nft_rewarder: AccountInfo<'info>,

    /// CHECK: Platform wallet — validated against ClearingHouseState
    #[account(
        mut,
        constraint = platform_wallet.key() == clearing_house.platform_wallet @ RogueTraderError::InvalidFeeWallet
    )]
    pub platform_wallet: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

/// Best-effort SOL transfer from vault PDA to recipient.
/// Emits FeeTransferFailed event on failure — fee stays in vault (benefits LP holders).
pub fn distribute_fee<'info>(
    vault: &AccountInfo<'info>,
    recipient: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    vault_seeds: &[&[u8]],
    fee: u64,
    share_bps: u16,
    total_bps: u16,
) {
    if share_bps == 0 || *recipient.key == Pubkey::default() || fee == 0 || total_bps == 0 {
        return;
    }
    let share = (fee as u128 * share_bps as u128 / total_bps as u128) as u64;
    if share == 0 {
        return;
    }
    match anchor_lang::solana_program::program::invoke_signed(
        &anchor_lang::solana_program::system_instruction::transfer(
            vault.key, recipient.key, share,
        ),
        &[vault.clone(), recipient.clone(), system_program.clone()],
        &[vault_seeds],
    ) {
        Ok(()) => {}
        Err(_) => {
            emit!(FeeTransferFailed {
                recipient: *recipient.key,
                amount: share,
                timestamp: Clock::get().map(|c| c.unix_timestamp).unwrap_or(0),
            });
        }
    }
}

pub fn handler(ctx: Context<DepositSol>, amount: u64) -> Result<()> {
    let deposit_fee_bps = ctx.accounts.clearing_house.deposit_fee_bps;
    let spread_to_lp_bps = ctx.accounts.clearing_house.spread_to_lp_bps;
    let referral_bps = ctx.accounts.clearing_house.referral_bps;
    let tier2_bps = ctx.accounts.clearing_house.tier2_referral_bps;
    let bonus_bps = ctx.accounts.clearing_house.bonus_bps;
    let nft_bps = ctx.accounts.clearing_house.nft_reward_bps;
    let platform_bps = ctx.accounts.clearing_house.platform_fee_bps;
    let vault_bump = ctx.accounts.clearing_house.vault_bump;

    // L-6: Use granular pause flag (deposits_paused) with legacy fallback
    require!(
        !ctx.accounts.clearing_house.deposits_paused && !ctx.accounts.clearing_house.paused,
        RogueTraderError::Paused
    );
    require!(amount > 0, RogueTraderError::ZeroAmount);

    let bot_id = ctx.accounts.agent_vault.bot_id;

    // Calculate spread: total_spread = fee portion, split between LP holders and fee wallets
    let total_spread = (amount as u128 * deposit_fee_bps as u128 / 10_000u128) as u64;
    let lp_spread = if deposit_fee_bps > 0 {
        (total_spread as u128 * spread_to_lp_bps as u128 / deposit_fee_bps as u128) as u64
    } else {
        0
    };
    let distribute_amount = total_spread.checked_sub(lp_spread).ok_or(RogueTraderError::MathOverflow)?;

    // vault_credit = amount minus only the wallet-distributed portion
    // lp_spread stays in vault (no LP minted for it → NAV increases for existing holders)
    let vault_credit = amount.checked_sub(distribute_amount).ok_or(RogueTraderError::MathOverflow)?;

    // LP tokens minted for amount minus TOTAL spread (user pays full spread)
    let lp_basis = amount.checked_sub(total_spread).ok_or(RogueTraderError::MathOverflow)?;

    // Transfer full amount from depositor to master vault
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.depositor.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
            },
        ),
        amount,
    )?;

    // Distribute wallet portion of fee (best-effort)
    let wallet_fee_bps = deposit_fee_bps.saturating_sub(spread_to_lp_bps);
    let vault_seeds: &[&[u8]] = &[b"vault", &[vault_bump]];
    let sys_info = ctx.accounts.system_program.to_account_info();

    let player_referrer = ctx.accounts.player_state.referrer;
    let player_tier2 = ctx.accounts.player_state.tier2_referrer;

    if player_referrer != Pubkey::default() {
        distribute_fee(&ctx.accounts.vault, &ctx.accounts.referrer, &sys_info, vault_seeds, distribute_amount, referral_bps, wallet_fee_bps);
    }
    if player_tier2 != Pubkey::default() {
        distribute_fee(&ctx.accounts.vault, &ctx.accounts.tier2_referrer, &sys_info, vault_seeds, distribute_amount, tier2_bps, wallet_fee_bps);
    }
    distribute_fee(&ctx.accounts.vault, &ctx.accounts.bonus_wallet, &sys_info, vault_seeds, distribute_amount, bonus_bps, wallet_fee_bps);
    distribute_fee(&ctx.accounts.vault, &ctx.accounts.nft_rewarder, &sys_info, vault_seeds, distribute_amount, nft_bps, wallet_fee_bps);
    distribute_fee(&ctx.accounts.vault, &ctx.accounts.platform_wallet, &sys_info, vault_seeds, distribute_amount, platform_bps, wallet_fee_bps);

    // Calculate LP tokens to mint using FULL sol_balance (not effective).
    // This assumes all active bets will win, giving depositors the highest
    // (worst for them) LP price — protects existing LP holders from arbitrage.
    let deposit_balance = ctx.accounts.agent_vault.sol_balance;
    let current_supply = ctx.accounts.agent_vault.total_lp_supply;

    let lp_to_mint = if current_supply == 0 {
        let lp = lp_basis;
        require!(lp > AgentVault::MINIMUM_LIQUIDITY, RogueTraderError::DepositTooSmall);
        lp - AgentVault::MINIMUM_LIQUIDITY
    } else {
        (lp_basis as u128)
            .checked_mul(current_supply as u128)
            .ok_or(RogueTraderError::MathOverflow)?
            .checked_div(deposit_balance as u128)
            .ok_or(RogueTraderError::MathOverflow)? as u64
    };

    require!(lp_to_mint > 0, RogueTraderError::DepositTooSmall);

    // Mint LP tokens
    let lp_auth_seeds: &[&[u8]] = &[b"bot_lp_authority", &[bot_id], &[ctx.accounts.agent_vault.lp_authority_bump]];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.lp_mint.to_account_info(),
                to: ctx.accounts.user_lp_account.to_account_info(),
                authority: ctx.accounts.lp_authority.to_account_info(),
            },
            &[lp_auth_seeds],
        ),
        lp_to_mint,
    )?;

    // Update vault bookkeeping — vault_credit includes lp_spread (stays in vault)
    let vault = &mut ctx.accounts.agent_vault;
    vault.sol_balance = vault.sol_balance.checked_add(vault_credit).ok_or(RogueTraderError::MathOverflow)?;
    if current_supply == 0 {
        vault.total_lp_supply = lp_to_mint + AgentVault::MINIMUM_LIQUIDITY;
    } else {
        vault.total_lp_supply = vault.total_lp_supply.checked_add(lp_to_mint).ok_or(RogueTraderError::MathOverflow)?;
    }
    vault.total_deposited = vault.total_deposited.checked_add(vault_credit).ok_or(RogueTraderError::MathOverflow)?;
    vault.deposit_count += 1;

    // Update player state (set wallet + bump on first init)
    let player = &mut ctx.accounts.player_state;
    if player.wallet == Pubkey::default() {
        player.wallet = ctx.accounts.depositor.key();
        player.bump = ctx.bumps.player_state;
    }
    player.total_deposited = player.total_deposited.checked_add(amount).ok_or(RogueTraderError::MathOverflow)?;
    player.deposit_count += 1;

    // Update clearing house fees — track total spread (LP + wallet portions)
    let ch = &mut ctx.accounts.clearing_house;
    ch.total_deposit_fees = ch.total_deposit_fees.checked_add(total_spread).ok_or(RogueTraderError::MathOverflow)?;

    let clock = Clock::get()?;
    emit!(DepositCompleted {
        depositor: ctx.accounts.depositor.key(),
        bot_id,
        sol_amount: amount,
        fee_amount: total_spread,
        lp_minted: lp_to_mint,
        new_sol_balance: vault.sol_balance,
        new_lp_supply: vault.total_lp_supply,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
