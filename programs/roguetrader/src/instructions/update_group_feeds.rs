use anchor_lang::prelude::*;
use crate::state::{ClearingHouseState, GroupConfig};
use crate::errors::RogueTraderError;

#[derive(Accounts)]
#[instruction(group_id: u8)]
pub struct UpdateGroupFeeds<'info> {
    #[account(
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
        has_one = authority,
    )]
    pub clearing_house: Account<'info, ClearingHouseState>,

    #[account(
        mut,
        seeds = [b"group_config", group_id.to_le_bytes().as_ref()],
        bump = group_config.bump,
    )]
    pub group_config: Account<'info, GroupConfig>,

    pub authority: Signer<'info>,
}

pub fn handler(
    ctx: Context<UpdateGroupFeeds>,
    _group_id: u8,
    pyth_feeds: Vec<Pubkey>,
    feed_count: u8,
) -> Result<()> {
    require!(pyth_feeds.len() <= GroupConfig::MAX_FEEDS, RogueTraderError::TooManyFeeds);
    require!(feed_count as usize == pyth_feeds.len(), RogueTraderError::InvalidConfig);

    let gc = &mut ctx.accounts.group_config;
    let mut feeds = [Pubkey::default(); 20];
    for (i, feed) in pyth_feeds.iter().enumerate() {
        feeds[i] = *feed;
    }
    gc.pyth_feeds = feeds;
    gc.feed_count = feed_count;

    Ok(())
}
