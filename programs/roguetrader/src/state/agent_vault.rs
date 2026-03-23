use anchor_lang::prelude::*;

#[account]
pub struct AgentVault {
    // Identity
    pub bot_id: u8,                     // 1
    pub group_id: u8,                   // 1
    pub name: [u8; 16],                 // 16

    // LP token
    pub lp_mint: Pubkey,                // 32 — SPL mint for this bot's LP token
    pub lp_authority: Pubkey,           // 32 — PDA that signs mint/burn
    pub lp_mint_bump: u8,              // 1
    pub lp_authority_bump: u8,         // 1

    // SOL balance tracking
    pub sol_balance: u64,               // 8 — total SOL in vault
    pub locked_sol: u64,                // 8 — total locked (proposer + counterparty)

    // LP tracking (cache — authoritative source is lp_mint.supply, updated atomically)
    pub total_lp_supply: u64,          // 8 — cached from mint (9 decimals)
    pub total_deposited: u64,          // 8 — lifetime SOL deposited by users
    pub total_withdrawn: u64,          // 8 — lifetime SOL withdrawn by users
    pub deposit_count: u64,            // 8
    pub withdrawal_count: u64,         // 8

    // Authorized executor
    pub authorized_executor: Pubkey,    // 32 — settler pubkey

    // Dynamic odds — rolling window (array is 100 bytes but WINDOW_SIZE controls active range)
    pub bet_window: [u8; 100],         // 100 — circular buffer (1=win, 0=loss)
    pub window_head: u8,               // 1 — next write position
    pub window_count: u8,              // 1 — bets recorded (0-WINDOW_SIZE)
    pub wins_in_window: u8,            // 1 — running win count

    // All-time stats
    pub bets_proposed: u32,            // 4
    pub bets_won: u32,                 // 4
    pub bets_lost: u32,                // 4
    pub bets_tied: u32,                // 4

    // Active bet tracking
    pub active_bet_count: u8,          // 1 — max 3

    pub bump: u8,                       // 1

    // Counterparty exposure tracking (added from _reserved bytes)
    pub counterparty_locked_sol: u64,  // 8 — subset of locked_sol: only CP exposure

    pub _reserved: [u8; 56],           // 56 (was 64, used 8 for counterparty_locked_sol)
}
// Total: ~430 bytes

impl AgentVault {
    pub const MAX_ACTIVE_BETS: u8 = 1;
    pub const DEFAULT_WINDOW_SIZE: u8 = 10;
    pub const MIN_STAKE: u64 = 10_000; // 0.00001 SOL
    pub const MINIMUM_LIQUIDITY: u64 = 10_000; // burned on first deposit

    /// Free capital available for new bets
    pub fn free_capital(&self) -> u64 {
        self.sol_balance.saturating_sub(self.locked_sol)
    }

    /// Effective balance for LP pricing (excludes locked capital)
    pub fn effective_balance(&self) -> u64 {
        self.sol_balance.saturating_sub(self.locked_sol)
    }

    /// Compute dynamic odds from rolling win rate.
    /// Returns (proposer_bps, counterparty_bps) out of 10,000.
    /// min/max_odds_bps from ClearingHouseState clamp the range.
    /// Must use u64 — wins_in_window (u8, max 100) × 10,000 = 1,000,000 overflows u16.
    pub fn compute_odds(&self, min_odds_bps: u16, max_odds_bps: u16, window_size: u8, invert: bool) -> (u64, u64) {
        let ws = if window_size == 0 { Self::DEFAULT_WINDOW_SIZE } else { window_size.min(100) };
        // Cap to window_size for transition safety (old vaults may have window_count > ws)
        let effective_count = (self.window_count as u64).min(ws as u64);
        let effective_wins = (self.wins_in_window as u64).min(effective_count);

        let win_rate_bps: u64 = if effective_count == 0 {
            5_000 // cold start: 50/50
        } else {
            effective_wins
                .checked_mul(10_000)
                .unwrap()
                / effective_count
        };

        // Invert: winning bots get better odds (lower p = bigger counterparty pool)
        let rate = if invert { 10_000u64.saturating_sub(win_rate_bps) } else { win_rate_bps };

        let floor = if min_odds_bps > 0 { min_odds_bps as u64 } else { 4_500 };
        let ceiling = if max_odds_bps > 0 { max_odds_bps as u64 } else { 5_500 };
        let p = rate.max(floor).min(ceiling);
        let q = 10_000 - p;

        (p, q)
    }

    /// Compute proposer stake and counterparty pool from Kelly bps and odds.
    /// stake_bps: Kelly input (0-500, capped at 500 = 5%)
    /// Returns (proposer_stake, cp_pool)
    pub fn apply_odds_to_stake(&self, stake_bps: u64, min_odds_bps: u16, max_odds_bps: u16, window_size: u8, invert: bool) -> (u64, u64) {
        let free = self.free_capital();
        let capped_bps = stake_bps.min(500);
        let proposer_stake = free
            .checked_mul(capped_bps)
            .unwrap()
            / 10_000;

        let (p, q) = self.compute_odds(min_odds_bps, max_odds_bps, window_size, invert);
        // cp_pool = proposer_stake × q / p
        let cp_pool = proposer_stake
            .checked_mul(q)
            .unwrap()
            / p;

        (proposer_stake, cp_pool)
    }

    /// Record a bet outcome in the rolling window.
    /// win: true = win (1), false = loss (0). Ties should NOT call this.
    /// M-7: Handles window size reduction by trimming stale data.
    pub fn update_win_rate(&mut self, win: bool, window_size: u8) {
        let ws = if window_size == 0 { Self::DEFAULT_WINDOW_SIZE } else { window_size.min(100) };

        // M-7: If window_count exceeds current window size (window was reduced),
        // trim stale data before recording the new outcome
        if self.window_count > ws {
            let mut valid_wins: u8 = 0;
            for j in 0..ws {
                let idx = if self.window_head >= j + 1 {
                    (self.window_head - j - 1) as usize
                } else {
                    (ws - (j + 1 - self.window_head)) as usize
                };
                if idx < 100 && self.bet_window[idx] == 1 {
                    valid_wins += 1;
                }
            }
            self.wins_in_window = valid_wins;
            self.window_count = ws;
            // Clear slots beyond new window size
            for j in (ws as usize)..100 {
                self.bet_window[j] = 0;
            }
            // Reset head to wrap within new size
            self.window_head = self.window_head % ws;
        }

        let idx = self.window_head as usize;

        if self.window_count >= ws {
            // Window is full — subtract the value being overwritten
            if self.bet_window[idx] == 1 {
                self.wins_in_window = self.wins_in_window.saturating_sub(1);
            }
        } else {
            self.window_count += 1;
        }

        // Write new value
        let val = if win { 1u8 } else { 0u8 };
        self.bet_window[idx] = val;
        if win {
            self.wins_in_window += 1;
        }

        // Advance head (circular)
        self.window_head = ((self.window_head as usize + 1) % ws as usize) as u8;
    }
}
