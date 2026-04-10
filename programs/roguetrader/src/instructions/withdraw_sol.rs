use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount};
use crate::state::{AgentVault, ClearingHouseState, PlayerState};
use crate::errors::RogueTraderError;
use crate::events::WithdrawCompleted;
use crate::instructions::deposit_sol::{distribute_fee_with_fallback, update_referral_earnings};

#[derive(Accounts)]
pub struct WithdrawSol<'info> {
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
    pub lp_mint: Box<Account<'info, Mint>>,

    /// User's LP token account (ATA) — I-6: validated mint and owner at Anchor level
    #[account(
        mut,
        token::mint = lp_mint,
        token::authority = withdrawer,
    )]
    pub user_lp_account: Box<Account<'info, TokenAccount>>,

    /// Player state for tracking (auto-created if needed)
    #[account(
        init_if_needed,
        payer = withdrawer,
        space = 8 + 193,
        seeds = [b"player_state", withdrawer.key().as_ref()],
        bump,
    )]
    pub player_state: Box<Account<'info, PlayerState>>,

    #[account(mut)]
    pub withdrawer: Signer<'info>,

    /// CHECK: Tier-1 referrer wallet — validated against PlayerState.
    /// When no referrer is set (default), accepts platform_wallet as writable substitute.
    #[account(
        mut,
        constraint = referrer.key() == player_state.referrer
            || (player_state.referrer == Pubkey::default() && referrer.key() == clearing_house.platform_wallet)
            @ RogueTraderError::InvalidFeeWallet
    )]
    pub referrer: AccountInfo<'info>,

    /// CHECK: Tier-2 referrer wallet — validated against PlayerState.
    /// When no tier-2 referrer is set (default), accepts platform_wallet as writable substitute.
    #[account(
        mut,
        constraint = tier2_referrer.key() == player_state.tier2_referrer
            || (player_state.tier2_referrer == Pubkey::default() && tier2_referrer.key() == clearing_house.platform_wallet)
            @ RogueTraderError::InvalidFeeWallet
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

    /// CHECK: Tier-1 referrer's ReferralState PDA — for updating total_earnings.
    /// When referrer is set, pass the PDA at [b"referral_state", referrer_wallet].
    /// When no referrer, pass any writable account (handler validates PDA before writing).
    #[account(mut)]
    pub referral_state: AccountInfo<'info>,

    /// CHECK: Tier-2 referrer's ReferralState PDA — same pattern.
    #[account(mut)]
    pub tier2_referral_state: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<WithdrawSol>, lp_amount: u64) -> Result<()> {
    let withdrawal_fee_bps = ctx.accounts.clearing_house.withdrawal_fee_bps;
    let spread_to_lp_bps = ctx.accounts.clearing_house.spread_to_lp_bps;
    let referral_bps = ctx.accounts.clearing_house.referral_bps;
    let tier2_bps = ctx.accounts.clearing_house.tier2_referral_bps;
    let bonus_bps = ctx.accounts.clearing_house.bonus_bps;
    let nft_bps = ctx.accounts.clearing_house.nft_reward_bps;
    let platform_bps = ctx.accounts.clearing_house.platform_fee_bps;
    let vault_bump = ctx.accounts.clearing_house.vault_bump;

    // L-6: Use granular pause flag (withdrawals_paused) with legacy fallback
    require!(
        !ctx.accounts.clearing_house.withdrawals_paused && !ctx.accounts.clearing_house.paused,
        RogueTraderError::Paused
    );
    require!(lp_amount > 0, RogueTraderError::ZeroAmount);

    let bot_id = ctx.accounts.agent_vault.bot_id;
    // Withdraw uses EFFECTIVE balance (sol_balance - locked_sol).
    // This assumes all active bets will lose, giving withdrawers the lowest
    // (worst for them) LP price — protects remaining LP holders from front-running.
    let withdraw_balance = ctx.accounts.agent_vault.effective_balance();
    let total_supply = ctx.accounts.agent_vault.total_lp_supply;

    require!(total_supply > 0, RogueTraderError::InsufficientLiquidity);

    // Calculate gross SOL at pessimistic price
    let gross_sol = (lp_amount as u128)
        .checked_mul(withdraw_balance as u128)
        .ok_or(RogueTraderError::MathOverflow)?
        .checked_div(total_supply as u128)
        .ok_or(RogueTraderError::MathOverflow)? as u64;

    require!(gross_sol > 0, RogueTraderError::WithdrawTooSmall);
    require!(gross_sol <= withdraw_balance, RogueTraderError::InsufficientFreeCapital);

    // Calculate spread: split between LP holders and fee wallets
    let total_spread = (gross_sol as u128 * withdrawal_fee_bps as u128 / 10_000u128) as u64;
    let lp_spread = if withdrawal_fee_bps > 0 {
        (total_spread as u128 * spread_to_lp_bps as u128 / withdrawal_fee_bps as u128) as u64
    } else {
        0
    };
    let distribute_amount = total_spread.checked_sub(lp_spread).ok_or(RogueTraderError::MathOverflow)?;
    let net_sol = gross_sol.checked_sub(total_spread).ok_or(RogueTraderError::MathOverflow)?;

    // Burn LP tokens from user
    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.lp_mint.to_account_info(),
                from: ctx.accounts.user_lp_account.to_account_info(),
                authority: ctx.accounts.withdrawer.to_account_info(),
            },
        ),
        lp_amount,
    )?;

    // Transfer net SOL from master vault to user
    let vault_seeds: &[&[u8]] = &[b"vault", &[vault_bump]];

    anchor_lang::solana_program::program::invoke_signed(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.vault.key(),
            &ctx.accounts.withdrawer.key(),
            net_sol,
        ),
        &[
            ctx.accounts.vault.to_account_info(),
            ctx.accounts.withdrawer.to_account_info(),
        ],
        &[vault_seeds],
    )?;

    // Distribute wallet portion of fee (best-effort, with accounting)
    let wallet_fee_bps = withdrawal_fee_bps.saturating_sub(spread_to_lp_bps);
    let sys_info = ctx.accounts.system_program.to_account_info();
    let player_referrer = ctx.accounts.player_state.referrer;
    let player_tier2 = ctx.accounts.player_state.tier2_referrer;

    // Distribute fees and track actual amounts paid (0 if transfer failed) plus whether
    // the share was redirected to platform due to the rent-safety fallback.
    let platform_info = ctx.accounts.platform_wallet.to_account_info();

    let (ref_paid, ref_redirected) = if player_referrer != Pubkey::default() {
        distribute_fee_with_fallback(
            &ctx.accounts.vault,
            &ctx.accounts.referrer,
            &platform_info,
            &sys_info, vault_seeds, distribute_amount, referral_bps, wallet_fee_bps,
        )
    } else {
        distribute_fee_with_fallback(
            &ctx.accounts.vault,
            &ctx.accounts.platform_wallet,
            &platform_info,
            &sys_info, vault_seeds, distribute_amount, referral_bps, wallet_fee_bps,
        )
    };
    let (t2_paid, t2_redirected) = if player_tier2 != Pubkey::default() {
        distribute_fee_with_fallback(
            &ctx.accounts.vault,
            &ctx.accounts.tier2_referrer,
            &platform_info,
            &sys_info, vault_seeds, distribute_amount, tier2_bps, wallet_fee_bps,
        )
    } else {
        distribute_fee_with_fallback(
            &ctx.accounts.vault,
            &ctx.accounts.platform_wallet,
            &platform_info,
            &sys_info, vault_seeds, distribute_amount, tier2_bps, wallet_fee_bps,
        )
    };
    let (bonus_paid, _) = distribute_fee_with_fallback(
        &ctx.accounts.vault, &ctx.accounts.bonus_wallet, &platform_info,
        &sys_info, vault_seeds, distribute_amount, bonus_bps, wallet_fee_bps,
    );
    let (nft_paid, _) = distribute_fee_with_fallback(
        &ctx.accounts.vault, &ctx.accounts.nft_rewarder, &platform_info,
        &sys_info, vault_seeds, distribute_amount, nft_bps, wallet_fee_bps,
    );
    let (plat_paid, _) = distribute_fee_with_fallback(
        &ctx.accounts.vault, &ctx.accounts.platform_wallet, &platform_info,
        &sys_info, vault_seeds, distribute_amount, platform_bps, wallet_fee_bps,
    );

    // Update per-referrer on-chain earnings (manual PDA validation + deserialization).
    // Skip when the share was redirected to platform — the referrer did not actually earn it.
    if ref_paid > 0 && player_referrer != Pubkey::default() && !ref_redirected {
        update_referral_earnings(&ctx.accounts.referral_state, &player_referrer, ctx.program_id, ref_paid);
    }
    if t2_paid > 0 && player_tier2 != Pubkey::default() && !t2_redirected {
        update_referral_earnings(&ctx.accounts.tier2_referral_state, &player_tier2, ctx.program_id, t2_paid);
    }

    // Update vault bookkeeping
    // Deduct gross_sol minus lp_spread — the lp_spread stays in vault, boosting remaining holders' NAV
    let vault = &mut ctx.accounts.agent_vault;
    let vault_debit = gross_sol.checked_sub(lp_spread).ok_or(RogueTraderError::MathOverflow)?;
    vault.sol_balance = vault.sol_balance.saturating_sub(vault_debit);
    vault.total_lp_supply = vault.total_lp_supply.saturating_sub(lp_amount);
    vault.total_withdrawn = vault.total_withdrawn.checked_add(gross_sol).ok_or(RogueTraderError::MathOverflow)?;
    vault.withdrawal_count += 1;

    // Update player state (set wallet + bump on first init)
    let player = &mut ctx.accounts.player_state;
    if player.wallet == Pubkey::default() {
        player.wallet = ctx.accounts.withdrawer.key();
        player.bump = ctx.bumps.player_state;
    }
    player.total_withdrawn = player.total_withdrawn.checked_add(gross_sol).ok_or(RogueTraderError::MathOverflow)?;
    player.withdrawal_count += 1;

    // Update clearing house fees — track total spread + per-category breakdown.
    // Only count fees as "referral" when they actually went to a referrer:
    //   - referrer was set (not default), AND
    //   - the share was NOT redirected to platform by the rent-safety fallback.
    // Anything that was supposed to go to a referrer but didn't, lands in platform totals.
    let actual_ref = if player_referrer != Pubkey::default() && !ref_redirected { ref_paid } else { 0 };
    let actual_t2 = if player_tier2 != Pubkey::default() && !t2_redirected { t2_paid } else { 0 };
    let fallback_plat = (ref_paid - actual_ref) + (t2_paid - actual_t2);

    let ch = &mut ctx.accounts.clearing_house;
    ch.total_withdrawal_fees = ch.total_withdrawal_fees.checked_add(total_spread).ok_or(RogueTraderError::MathOverflow)?;
    ch.total_referral_paid = ch.total_referral_paid.saturating_add(actual_ref).saturating_add(actual_t2);
    ch.total_bonus_paid = ch.total_bonus_paid.saturating_add(bonus_paid);
    ch.total_nft_rewards_paid = ch.total_nft_rewards_paid.saturating_add(nft_paid);
    ch.total_platform_fees_paid = ch.total_platform_fees_paid.saturating_add(plat_paid).saturating_add(fallback_plat);

    let clock = Clock::get()?;
    emit!(WithdrawCompleted {
        withdrawer: ctx.accounts.withdrawer.key(),
        bot_id,
        lp_burned: lp_amount,
        sol_returned: net_sol,
        fee_amount: total_spread,
        new_sol_balance: vault.sol_balance,
        new_lp_supply: vault.total_lp_supply,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
