use anchor_lang::prelude::*;

#[account]
pub struct GroupConfig {
    pub group_id: u8,                   // 1
    pub name: [u8; 32],                 // 32
    pub pyth_feeds: [Pubkey; 20],      // 640 — stores Hermes feed IDs as Pubkey (32 bytes each)
    pub feed_count: u8,                 // 1
    pub bump: u8,                       // 1
    pub _reserved: [u8; 32],           // 32
}
// Total: ~707 bytes
// Note: pyth_feeds stores Hermes feed IDs (32-byte hashes) cast as Pubkey for storage.
// Validation compares against the feed_id field inside PriceUpdateV2 data, NOT the account pubkey.

impl GroupConfig {
    pub const MAX_FEEDS: usize = 20;

    /// Check if a Pyth Hermes feed ID is assigned to this group.
    /// Compares the 32-byte feed_id from PriceUpdateV2 against stored feed IDs.
    pub fn has_feed_id(&self, feed_id: &[u8; 32]) -> bool {
        self.pyth_feeds[..self.feed_count as usize]
            .iter()
            .any(|f| f.to_bytes() == *feed_id)
    }
}
