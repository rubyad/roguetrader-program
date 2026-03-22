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
}
