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
    pub lp_mint: Box<Account<'info, Mint>>,

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
    pub user_lp_account: Box<Account<'info, TokenAccount>>,

    /// Player state for referral tracking (auto-created on first deposit)
    #[account(
        init_if_needed,
        payer = depositor,
        space = 8 + 193,
        seeds = [b"player_state", depositor.key().as_ref()],
        bump,
    )]
    pub player_state: Box<Account<'info, PlayerState>>,

    #[account(mut)]
    pub depositor: Signer<'info>,

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

/// Best-effort SOL transfer from vault PDA to recipient, with a rent-safety fallback.
///
/// Returns `(paid_amount, redirected_to_fallback)`:
/// - `paid_amount`: actual lamports transferred (0 on failure or skip).
/// - `redirected_to_fallback`: true iff the share was routed to `fallback` instead of
///   `recipient` because sending to `recipient` would have created a rent-paying account
///   (Uninitialized → RentPaying, forbidden by SIMD-0058).
///
/// The redirect only triggers when `recipient_pre == 0` AND `recipient_pre + share <
/// rent_exempt_min`. Transitions of the form `RentPaying → RentPaying` are allowed by the
/// runtime, so partially-funded wallets still receive shares normally.
///
/// Emits `FeeTransferFailed` on CPI failure — fee stays in vault (benefits LP holders).
pub fn distribute_fee_with_fallback<'info>(
    vault: &AccountInfo<'info>,
    recipient: &AccountInfo<'info>,
    fallback: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    vault_seeds: &[&[u8]],
    fee: u64,
    share_bps: u16,
    total_bps: u16,
) -> (u64, bool) {
    if share_bps == 0 || *recipient.key == Pubkey::default() || fee == 0 || total_bps == 0 {
        return (0, false);
    }
    let share = (fee as u128 * share_bps as u128 / total_bps as u128) as u64;
    if share == 0 {
        return (0, false);
    }

    // Decide target: recipient if it stays valid, else fallback.
    let recipient_is_self = recipient.key() == fallback.key();
    let target = if recipient_is_self {
        // Already pointing at the fallback (e.g., no-referrer case) — no redirect possible.
        recipient
    } else {
        // Rent-exempt minimum for a 0-byte system account. `Rent::get()` only fails if the
        // sysvar isn't loaded, which shouldn't happen mid-instruction; 890_880 is the
        // documented constant for an empty system account at current protocol parameters.
        let rent_exempt_min: u64 = Rent::get()
            .map(|r| r.minimum_balance(0))
            .unwrap_or(890_880);
        let recipient_pre = recipient.lamports();
        let recipient_post = recipient_pre.saturating_add(share);
        let pre_was_uninit = recipient_pre == 0;
        let post_would_be_rent_paying = recipient_post < rent_exempt_min;
        if pre_was_uninit && post_would_be_rent_paying {
            // Forbidden Uninitialized → RentPaying transition — route to fallback instead
            // so the whole TX doesn't fail with InsufficientFundsForRent (SIMD-0058).
            fallback
        } else {
            recipient
        }
    };

    let was_redirected = target.key() != recipient.key();

    match anchor_lang::solana_program::program::invoke_signed(
        &anchor_lang::solana_program::system_instruction::transfer(
            vault.key, target.key, share,
        ),
        &[vault.clone(), target.clone(), system_program.clone()],
        &[vault_seeds],
    ) {
        Ok(()) => (share, was_redirected),
        Err(_) => {
            emit!(FeeTransferFailed {
                recipient: *target.key,
                amount: share,
                timestamp: Clock::get().map(|c| c.unix_timestamp).unwrap_or(0),
            });
            (0, false)
        }
    }
}

/// Best-effort update of ReferralState.total_earnings.
/// Validates PDA, owner, and discriminator before writing.
/// Silently skips if account doesn't match (no error, no revert).
pub fn update_referral_earnings(
    account: &AccountInfo,
    referrer_key: &Pubkey,
    program_id: &Pubkey,
    amount: u64,
) {
    // Validate PDA derivation
    let (expected_pda, _) = Pubkey::find_program_address(
        &[b"referral_state", referrer_key.as_ref()],
        program_id,
    );
    if account.key != &expected_pda {
        return;
    }
    // Validate owner
    if account.owner != program_id {
        return;
    }
    // Validate data length (8 discriminator + ReferralState fields)
    let data_len = account.data_len();
    if data_len < 8 + 32 + 8 + 8 + 1 {
        return;
    }
    // Update total_earnings in-place (offset: 8 disc + 32 referrer = 40)
    if let Ok(mut data) = account.try_borrow_mut_data() {
        let offset = 8 + 32; // discriminator + referrer Pubkey
        let current = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap_or([0; 8]));
        let new_val = current.saturating_add(amount);
        data[offset..offset + 8].copy_from_slice(&new_val.to_le_bytes());
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
    msg!("fee_accounting: starting fee distribution");
    let wallet_fee_bps = deposit_fee_bps.saturating_sub(spread_to_lp_bps);
    let vault_seeds: &[&[u8]] = &[b"vault", &[vault_bump]];
    let sys_info = ctx.accounts.system_program.to_account_info();

    let player_referrer = ctx.accounts.player_state.referrer;
    let player_tier2 = ctx.accounts.player_state.tier2_referrer;

    msg!("fee_accounting: about to distribute fees");
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

    // Update clearing house fees — track total spread + per-category breakdown.
    // Only count fees as "referral" when they actually went to a referrer:
    //   - referrer was set (not default), AND
    //   - the share was NOT redirected to platform by the rent-safety fallback.
    // Anything that was supposed to go to a referrer but didn't, lands in platform totals.
    let actual_ref = if player_referrer != Pubkey::default() && !ref_redirected { ref_paid } else { 0 };
    let actual_t2 = if player_tier2 != Pubkey::default() && !t2_redirected { t2_paid } else { 0 };
    let fallback_plat = (ref_paid - actual_ref) + (t2_paid - actual_t2);

    let ch = &mut ctx.accounts.clearing_house;
    ch.total_deposit_fees = ch.total_deposit_fees.checked_add(total_spread).ok_or(RogueTraderError::MathOverflow)?;
    ch.total_referral_paid = ch.total_referral_paid.saturating_add(actual_ref).saturating_add(actual_t2);
    ch.total_bonus_paid = ch.total_bonus_paid.saturating_add(bonus_paid);
    ch.total_nft_rewards_paid = ch.total_nft_rewards_paid.saturating_add(nft_paid);
    ch.total_platform_fees_paid = ch.total_platform_fees_paid.saturating_add(plat_paid).saturating_add(fallback_plat);

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
