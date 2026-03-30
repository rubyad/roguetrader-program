use anchor_lang::prelude::*;

#[error_code]
pub enum RogueTraderError {
    #[msg("Protocol is paused")]
    Paused,                                 // 6000

    #[msg("Amount cannot be zero")]
    ZeroAmount,                             // 6001

    #[msg("Deposit too small")]
    DepositTooSmall,                        // 6002

    #[msg("Withdrawal too small")]
    WithdrawTooSmall,                       // 6003

    #[msg("Insufficient liquidity")]
    InsufficientLiquidity,                  // 6004

    #[msg("Math overflow")]
    MathOverflow,                           // 6005

    #[msg("Invalid configuration")]
    InvalidConfig,                          // 6006

    #[msg("Invalid bot ID")]
    InvalidBotId,                           // 6007

    #[msg("Invalid group ID")]
    InvalidGroupId,                         // 6008

    #[msg("Maximum active bets reached")]
    MaxActiveBetsReached,                   // 6009

    #[msg("Stake below minimum")]
    StakeBelowMinimum,                      // 6010

    #[msg("Insufficient counterparty liquidity")]
    InsufficientCounterpartyLiquidity,      // 6011

    #[msg("Pyth price too stale")]
    PythPriceTooStale,                      // 6012

    #[msg("Pyth confidence interval too wide")]
    PythConfidenceTooWide,                  // 6013

    #[msg("Pyth feed not in group")]
    PythFeedNotInGroup,                     // 6014

    #[msg("Bet not expired yet")]
    BetNotExpired,                          // 6015

    #[msg("Bet already settled")]
    BetAlreadySettled,                      // 6016

    #[msg("Bet not settled")]
    BetNotSettled,                          // 6017

    #[msg("Feed mismatch on settlement")]
    FeedMismatch,                           // 6018

    #[msg("Exponent changed between entry and exit")]
    ExponentChanged,                        // 6019

    #[msg("Invalid direction")]
    InvalidDirection,                       // 6020

    #[msg("Unauthorized settler")]
    UnauthorizedSettler,                    // 6021

    #[msg("Invalid referrer")]
    InvalidReferrer,                        // 6022

    #[msg("Self referral not allowed")]
    SelfReferral,                           // 6023

    #[msg("Referrer already set")]
    ReferrerAlreadySet,                     // 6024

    #[msg("Insufficient free capital for withdrawal")]
    InsufficientFreeCapital,                // 6025

    #[msg("Stale bet buffer not elapsed")]
    StaleBetBufferNotElapsed,               // 6026

    #[msg("Invalid fee split — components must sum to total")]
    InvalidFeeSplit,                        // 6027

    #[msg("Too many feeds for group")]
    TooManyFeeds,                           // 6028

    #[msg("Counterparty count mismatch")]
    CounterpartyCountMismatch,              // 6029

    #[msg("Invalid LP amount")]
    InvalidLpAmount,                        // 6030

    // === Security audit fixes (H-1, H-2, H-3, M-2, M-3, M-4, M-5, M-6) ===

    #[msg("Fee wallet does not match expected address")]
    InvalidFeeWallet,                       // 6031

    #[msg("Proposer vault bot_id does not match bet proposer")]
    VaultBetMismatch,                       // 6032

    #[msg("Pyth price feed account is not owned by the Pyth Receiver program")]
    InvalidPythAccount,                     // 6033

    #[msg("Fee splits do not match withdrawal fee configuration")]
    InvalidWithdrawalFeeSplit,              // 6034

    #[msg("Settlement window has expired — use expire_stale_bet instead")]
    SettlementWindowExpired,                // 6035

    #[msg("Counterparty vault PDA does not match expected derivation")]
    InvalidCounterpartyVault,               // 6036

    #[msg("Duplicate counterparty bot_id in remaining_accounts")]
    DuplicateCounterparty,                  // 6037

    #[msg("Proposer cannot be its own counterparty")]
    SelfCounterparty,                       // 6038

    #[msg("Missing required counterparty vault for settlement")]
    MissingCounterparty,                    // 6039

    #[msg("Oracle price is non-positive")]
    InvalidPrice,                           // 6040

    #[msg("Bet duration must be between 30 seconds and 24 hours")]
    InvalidDuration,                        // 6041

    #[msg("Unauthorized — only authority or settler may call this")]
    Unauthorized,                           // 6042

    #[msg("Pending authority does not match signer")]
    InvalidPendingAuthority,                // 6043

    #[msg("No pending authority transfer")]
    NoPendingTransfer,                      // 6044

    // === Rewards Pool / Raffle ===

    #[msg("Raffle is paused")]
    RafflePaused,                           // 6045

    #[msg("Rewards pool is empty")]
    EmptyRewardsPool,                       // 6046

    #[msg("Raffle interval has not elapsed")]
    RaffleTooEarly,                         // 6047
}
