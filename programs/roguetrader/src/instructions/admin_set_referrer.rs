use anchor_lang::prelude::*;
use crate::state::{ClearingHouseState, PlayerState, ReferralState};
use crate::errors::RogueTraderError;
use crate::events::ReferrerSet;

#[derive(Accounts)]
#[instruction(player_key: Pubkey, referrer_key: Pubkey)]
pub struct AdminSetReferrer<'info> {
    #[account(
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
    )]
    pub clearing_house: Account<'info, ClearingHouseState>,

    /// Player's state — init_if_needed (settler pays rent)
    #[account(
        init_if_needed,
        payer = settler,
        space = 8 + std::mem::size_of::<PlayerState>(),
        seeds = [b"player_state", player_key.as_ref()],
        bump,
    )]
    pub player_state: Account<'info, PlayerState>,

    /// Referrer's referral state — init_if_needed (settler pays rent)
    #[account(
        init_if_needed,
        payer = settler,
        space = 8 + std::mem::size_of::<ReferralState>(),
        seeds = [b"referral_state", referrer_key.as_ref()],
        bump,
    )]
    pub referral_state: Account<'info, ReferralState>,

    /// Referrer's player state (for tier-2 resolution)
    /// CHECK: May not exist
    pub referrer_player_state: AccountInfo<'info>,

    /// CHECK: Player wallet (not signing)
    pub player: AccountInfo<'info>,

    /// CHECK: Referrer wallet
    pub referrer: AccountInfo<'info>,

    /// Settler signer
    #[account(
        mut,
        constraint = settler.key() == clearing_house.settler @ RogueTraderError::UnauthorizedSettler,
    )]
    pub settler: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<AdminSetReferrer>,
    player_key: Pubkey,
    referrer_key: Pubkey,
) -> Result<()> {
    // Guards
    require!(player_key != referrer_key, RogueTraderError::SelfReferral);
    require!(referrer_key != Pubkey::default(), RogueTraderError::InvalidReferrer);

    let player_state = &mut ctx.accounts.player_state;

    // Initialize wallet if first time
    if player_state.wallet == Pubkey::default() {
        player_state.wallet = player_key;
        player_state.bump = ctx.bumps.player_state;
    }

    // Only set if not already set
    require!(player_state.referrer == Pubkey::default(), RogueTraderError::ReferrerAlreadySet);

    player_state.referrer = referrer_key;

    // Try tier-2 resolution from referrer's player state (if it exists and has data)
    let referrer_ps_info = &ctx.accounts.referrer_player_state;
    if referrer_ps_info.data_len() > 8 + 32 && referrer_ps_info.owner == &crate::ID {
        // Read the referrer field directly from raw data (offset 8 discriminator + 32 wallet = 40, referrer at 40)
        let data = referrer_ps_info.try_borrow_data()?;
        if data.len() >= 72 {
            let referrer_of_referrer = Pubkey::try_from(&data[40..72])
                .unwrap_or_default();
            if referrer_of_referrer != Pubkey::default() {
                player_state.tier2_referrer = referrer_of_referrer;
            }
        }
    }

    // Initialize referral state if first time
    let referral_state = &mut ctx.accounts.referral_state;
    if referral_state.referrer == Pubkey::default() {
        referral_state.referrer = referrer_key;
        referral_state.bump = ctx.bumps.referral_state;
    }
    referral_state.referral_count += 1;

    let clock = Clock::get()?;
    emit!(ReferrerSet {
        player: player_key,
        referrer: referrer_key,
        tier2_referrer: player_state.tier2_referrer,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
