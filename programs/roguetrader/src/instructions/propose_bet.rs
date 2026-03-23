use anchor_lang::prelude::*;
use crate::state::{
    AgentVault, Bet, ClearingHouseState, GroupConfig,
    bet::{CounterpartyPosition, Direction, MAX_COUNTERPARTIES},
};
use crate::errors::RogueTraderError;
use crate::events::BetProposed;
use crate::pyth::PriceUpdateV2;

/// Max Pyth price staleness in seconds
const MAX_PRICE_AGE: u64 = 60;
/// Max confidence interval as percentage of price (2% = 200 bps)
const MAX_CONF_BPS: u64 = 200;
/// M-6: Minimum bet duration (30 seconds)
const MIN_BET_DURATION: i64 = 30;
/// M-6: Maximum bet duration (24 hours)
const MAX_BET_DURATION: i64 = 86_400;

#[derive(Accounts)]
pub struct ProposeBet<'info> {
    #[account(
        mut,
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
    )]
    pub clearing_house: Box<Account<'info, ClearingHouseState>>,

    /// Proposer bot's AgentVault
    #[account(
        mut,
        seeds = [b"agent_vault", proposer_vault.bot_id.to_le_bytes().as_ref()],
        bump = proposer_vault.bump,
    )]
    pub proposer_vault: Box<Account<'info, AgentVault>>,

    /// Bet account (init, PDA from bet_id)
    #[account(
        init,
        payer = settler,
        space = 8 + std::mem::size_of::<Bet>(),
        seeds = [b"bet", clearing_house.next_bet_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub bet: Box<Account<'info, Bet>>,

    /// Pyth price feed account — must be owned by Pyth Receiver program
    /// CHECK: Owner validated here; discriminator/feed/staleness/confidence validated in handler
    #[account(
        owner = crate::pyth::PYTH_RECEIVER_PROGRAM_ID @ RogueTraderError::InvalidPythAccount
    )]
    pub pyth_price_feed: AccountInfo<'info>,

    /// Group config for feed validation
    #[account(
        seeds = [b"group_config", proposer_vault.group_id.to_le_bytes().as_ref()],
        bump = group_config.bump,
    )]
    pub group_config: Box<Account<'info, GroupConfig>>,

    /// Settler signer
    #[account(
        mut,
        constraint = settler.key() == clearing_house.settler @ RogueTraderError::UnauthorizedSettler,
    )]
    pub settler: Signer<'info>,

    pub system_program: Program<'info, System>,

    // 29 counterparty AgentVault accounts passed as remaining_accounts
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, ProposeBet<'info>>,
    direction: u8,
    stake_bps: u64,
    duration_seconds: i64,
) -> Result<()> {
    // Guards
    // L-6: Use granular pause flag (betting_paused) with legacy fallback
    require!(
        !ctx.accounts.clearing_house.betting_paused && !ctx.accounts.clearing_house.paused,
        RogueTraderError::Paused
    );
    require!(
        ctx.accounts.proposer_vault.active_bet_count < AgentVault::MAX_ACTIVE_BETS,
        RogueTraderError::MaxActiveBetsReached
    );
    // M-6: Validate bet duration bounds
    require!(
        duration_seconds >= MIN_BET_DURATION && duration_seconds <= MAX_BET_DURATION,
        RogueTraderError::InvalidDuration
    );

    let dir = match direction {
        0 => Direction::Long,
        1 => Direction::Short,
        _ => return Err(RogueTraderError::InvalidDirection.into()),
    };

    // Read Pyth price — extract values before releasing borrow
    let clock = Clock::get()?;
    let (entry_price, entry_conf, entry_expo) = {
        let price_data = ctx.accounts.pyth_price_feed.try_borrow_data()?;
        let price_update = PriceUpdateV2::try_deserialize(&price_data)?;

        // Validate Pyth feed ID is in proposer's group
        require!(
            ctx.accounts.group_config.has_feed_id(&price_update.price_message.feed_id),
            RogueTraderError::PythFeedNotInGroup
        );

        let price_msg = price_update
            .get_price_no_older_than(&clock, MAX_PRICE_AGE)?;

        // M-5: Reject non-positive prices (all RogueTrader assets are always positive)
        require!(price_msg.price > 0, RogueTraderError::InvalidPrice);
        let abs_price = price_msg.price as u64; // Safe after positive check

        // Validate confidence interval < 2% of price
        let conf_bps = (price_msg.conf as u128)
            .checked_mul(10_000)
            .unwrap()
            / (abs_price as u128);
        require!(conf_bps <= MAX_CONF_BPS as u128, RogueTraderError::PythConfidenceTooWide);

        (price_msg.price, price_msg.conf, price_msg.exponent)
    };

    // Compute odds and stake
    let min_odds = ctx.accounts.clearing_house.min_odds_bps;
    let max_odds = ctx.accounts.clearing_house.max_odds_bps;
    let ws = ctx.accounts.clearing_house.odds_window_size;
    let inv = ctx.accounts.clearing_house.invert_odds;
    let max_cp_bps = ctx.accounts.clearing_house.max_cp_exposure_bps;
    let (p, q) = ctx.accounts.proposer_vault.compute_odds(min_odds, max_odds, ws, inv);
    let win_rate_bps = p;
    let (mut proposer_stake, mut cp_pool_target) = ctx.accounts.proposer_vault.apply_odds_to_stake(stake_bps, min_odds, max_odds, ws, inv);
    let pyth_feed_key = ctx.accounts.pyth_price_feed.key();
    let proposer_bot_id = ctx.accounts.proposer_vault.bot_id;

    require!(proposer_stake >= AgentVault::MIN_STAKE, RogueTraderError::StakeBelowMinimum);

    // Process 29 counterparty AgentVaults from remaining_accounts
    require!(ctx.remaining_accounts.len() == MAX_COUNTERPARTIES, RogueTraderError::CounterpartyCountMismatch);

    // First pass: sum total free capital and compute capped total
    // M-4: Validate PDA derivation, uniqueness, and no self-counterparty
    let mut total_free: u64 = 0;
    let mut cp_frees: [u64; 29] = [0u64; 29];
    let mut cp_caps: [u64; 29] = [0u64; 29];
    let mut seen_bot_ids: [bool; 31] = [false; 31];

    for (i, acct) in ctx.remaining_accounts.iter().enumerate() {
        let cp_vault = Account::<AgentVault>::try_from(acct)?;

        // Verify this is a real AgentVault PDA
        let (expected_pda, _) = Pubkey::find_program_address(
            &[b"agent_vault", cp_vault.bot_id.to_le_bytes().as_ref()],
            &crate::ID,
        );
        require!(
            acct.key() == expected_pda,
            RogueTraderError::InvalidCounterpartyVault
        );

        // No duplicate bot_ids
        let bid = cp_vault.bot_id as usize;
        require!(bid <= 30, RogueTraderError::InvalidCounterpartyVault);
        require!(!seen_bot_ids[bid], RogueTraderError::DuplicateCounterparty);
        seen_bot_ids[bid] = true;

        // Counterparty must not be the proposer
        require!(
            cp_vault.bot_id != proposer_bot_id,
            RogueTraderError::SelfCounterparty
        );

        let cp_free = cp_vault.free_capital();
        cp_frees[i] = cp_free;
        total_free = total_free
            .checked_add(cp_free)
            .ok_or(RogueTraderError::MathOverflow)?;

        // Per-CP cap: cp_free × max_cp_bps / 10_000 (0 = disabled)
        cp_caps[i] = if max_cp_bps > 0 {
            (cp_free as u128)
                .checked_mul(max_cp_bps as u128)
                .unwrap()
                / 10_000u128
        } else {
            u64::MAX as u128
        } as u64;
    }

    // Compute capped total if exposure cap is active
    if max_cp_bps > 0 && total_free > 0 {
        let mut capped_total: u64 = 0;
        for i in 0..29 {
            if cp_frees[i] == 0 { continue; }
            let proportional = (cp_frees[i] as u128)
                .checked_mul(cp_pool_target as u128)
                .unwrap()
                / (total_free as u128);
            let capped = (proportional as u64).min(cp_caps[i]);
            capped_total = capped_total
                .checked_add(capped)
                .ok_or(RogueTraderError::MathOverflow)?;
        }

        if capped_total < cp_pool_target {
            // Scale down: actual_proposer_stake = capped_total × p / q
            let scaled = (capped_total as u128)
                .checked_mul(p as u128)
                .unwrap()
                / (q as u128);
            proposer_stake = u64::try_from(scaled).map_err(|_| RogueTraderError::MathOverflow)?;
            cp_pool_target = capped_total;

            require!(proposer_stake >= AgentVault::MIN_STAKE, RogueTraderError::StakeBelowMinimum);
        }
    }

    require!(total_free >= cp_pool_target, RogueTraderError::InsufficientCounterpartyLiquidity);

    // Second pass: distribute proportional stakes (capped) and lock capital
    let mut counterparties = [CounterpartyPosition::default(); 29];
    let mut cp_count: u8 = 0;
    let mut actual_cp_pool: u64 = 0;

    for (i, acct) in ctx.remaining_accounts.iter().enumerate() {
        let mut cp_vault = Account::<AgentVault>::try_from(acct)?;
        let cp_free = cp_frees[i];

        if cp_free == 0 {
            counterparties[cp_count as usize] = CounterpartyPosition {
                bot_id: cp_vault.bot_id,
                stake: 0,
            };
        } else {
            // cp_stake = min((cp_free / total_free) × cp_pool_target, cp_cap)
            let proportional = (cp_free as u128)
                .checked_mul(cp_pool_target as u128)
                .unwrap()
                / (total_free as u128);
            let cp_stake = (proportional as u64).min(cp_caps[i]);

            cp_vault.locked_sol = cp_vault
                .locked_sol
                .checked_add(cp_stake)
                .ok_or(RogueTraderError::MathOverflow)?;
            cp_vault.counterparty_locked_sol = cp_vault
                .counterparty_locked_sol
                .checked_add(cp_stake)
                .ok_or(RogueTraderError::MathOverflow)?;

            counterparties[cp_count as usize] = CounterpartyPosition {
                bot_id: cp_vault.bot_id,
                stake: cp_stake,
            };
            actual_cp_pool = actual_cp_pool
                .checked_add(cp_stake)
                .ok_or(RogueTraderError::MathOverflow)?;
        }

        // Write back modified counterparty vault
        cp_vault.exit(&crate::ID)?;
        cp_count += 1;
    }

    // Lock proposer's capital
    let proposer = &mut ctx.accounts.proposer_vault;
    proposer.locked_sol = proposer
        .locked_sol
        .checked_add(proposer_stake)
        .ok_or(RogueTraderError::MathOverflow)?;
    proposer.active_bet_count += 1;
    proposer.bets_proposed += 1;

    // Fill bet account
    let clearing_house = &mut ctx.accounts.clearing_house;
    let bet_id = clearing_house.next_bet_id;

    let bet = &mut ctx.accounts.bet;
    bet.bet_id = bet_id;
    bet.proposer_bot = proposer_bot_id;
    bet.pyth_feed = pyth_feed_key;
    bet.direction = dir;
    bet.duration_seconds = duration_seconds;
    bet.proposer_stake = proposer_stake;
    bet.counterparty_pool = actual_cp_pool;
    bet.win_rate_bps_at_open = win_rate_bps as u16;
    bet.counterparties = counterparties;
    bet.cp_count = cp_count;
    bet.entry_price = entry_price;
    bet.entry_conf = entry_conf;
    bet.entry_expo = entry_expo;
    bet.entry_timestamp = clock.unix_timestamp;
    bet.expiry_timestamp = clock
        .unix_timestamp
        .checked_add(duration_seconds)
        .ok_or(RogueTraderError::MathOverflow)?;
    bet.settled = false;
    bet.outcome = 0; // pending
    bet.exit_price = 0;
    bet.exit_conf = 0;
    bet.settle_timestamp = 0;
    bet.bump = ctx.bumps.bet;
    bet._reserved = [0u8; 32];

    // Update global state
    clearing_house.next_bet_id = clearing_house
        .next_bet_id
        .checked_add(1)
        .ok_or(RogueTraderError::MathOverflow)?;
    clearing_house.total_bets_proposed = clearing_house
        .total_bets_proposed
        .checked_add(1)
        .ok_or(RogueTraderError::MathOverflow)?;
    clearing_house.total_volume = clearing_house
        .total_volume
        .checked_add(proposer_stake)
        .and_then(|v| v.checked_add(actual_cp_pool))
        .ok_or(RogueTraderError::MathOverflow)?;

    emit!(BetProposed {
        bet_id: bet.bet_id,
        proposer_bot: bet.proposer_bot,
        pyth_feed: bet.pyth_feed,
        direction: direction,
        proposer_stake,
        counterparty_pool: actual_cp_pool,
        win_rate_bps: bet.win_rate_bps_at_open,
        entry_price: bet.entry_price,
        entry_conf: bet.entry_conf,
        duration_seconds,
        expiry_timestamp: bet.expiry_timestamp,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
