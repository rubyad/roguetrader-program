use anchor_lang::prelude::*;
use crate::state::{ClearingHouseState, GroupConfig};
use crate::errors::RogueTraderError;
use crate::events::GroupConfigCreated;

#[derive(Accounts)]
#[instruction(group_id: u8)]
pub struct CreateGroupConfig<'info> {
    #[account(
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
        has_one = authority,
    )]
    pub clearing_house: Account<'info, ClearingHouseState>,

    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<GroupConfig>(),
        seeds = [b"group_config", group_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub group_config: Account<'info, GroupConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<CreateGroupConfig>,
    group_id: u8,
    name: [u8; 32],
    pyth_feeds: Vec<Pubkey>,
) -> Result<()> {
    require!(group_id >= 1 && group_id <= 5, RogueTraderError::InvalidGroupId);
    require!(pyth_feeds.len() <= GroupConfig::MAX_FEEDS, RogueTraderError::TooManyFeeds);

    let gc = &mut ctx.accounts.group_config;
    gc.group_id = group_id;
    gc.name = name;

    // Copy feeds into fixed-size array
    let mut feeds = [Pubkey::default(); 20];
    for (i, feed) in pyth_feeds.iter().enumerate() {
        feeds[i] = *feed;
    }
    gc.pyth_feeds = feeds;
    gc.feed_count = pyth_feeds.len() as u8;
    gc.bump = ctx.bumps.group_config;
    gc._reserved = [0u8; 32];

    let clock = Clock::get()?;
    emit!(GroupConfigCreated {
        group_id,
        name,
        feed_count: gc.feed_count,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
