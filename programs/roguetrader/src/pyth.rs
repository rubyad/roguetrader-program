use anchor_lang::prelude::*;

/// Pyth PriceUpdateV2 account discriminator (first 8 bytes)
/// Matches the Anchor discriminator for the PriceUpdateV2 account from pyth-solana-receiver
pub const PRICE_UPDATE_V2_DISCRIMINATOR: [u8; 8] = [34, 241, 35, 99, 157, 126, 244, 205];

/// Pyth price message — core price data extracted from PriceUpdateV2 account
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct PriceFeedMessage {
    pub feed_id: [u8; 32],
    pub price: i64,
    pub conf: u64,
    pub exponent: i32,
    pub publish_time: i64,
    pub prev_publish_time: i64,
    pub ema_price: i64,
    pub ema_conf: u64,
}

/// Verification level enum matching Pyth's on-chain format
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum VerificationLevel {
    Partial { num_signatures: u8 },
    Full,
}

/// Local representation of Pyth PriceUpdateV2 account data
/// Layout: discriminator (8) + write_authority (32) + verification_level (enum) + price_message + posted_slot (8)
///
/// We deserialize this manually from AccountInfo rather than using the pyth SDK,
/// because pyth-solana-receiver-sdk v0.4 has irreconcilable dependency conflicts
/// with anchor-lang 0.30.1 (it pulls anchor-lang 0.32.1 via pythnet-sdk).
#[derive(Clone, Debug)]
pub struct PriceUpdateV2 {
    pub write_authority: Pubkey,
    pub verification_level: VerificationLevel,
    pub price_message: PriceFeedMessage,
    pub posted_slot: u64,
}

impl PriceUpdateV2 {
    /// Deserialize a PriceUpdateV2 from an AccountInfo's data.
    /// Validates the discriminator matches Pyth's PriceUpdateV2.
    pub fn try_deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < 8 {
            return Err(ProgramError::InvalidAccountData.into());
        }

        // Check discriminator
        if data[..8] != PRICE_UPDATE_V2_DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData.into());
        }

        let mut offset = 8;

        // write_authority: Pubkey (32 bytes)
        let write_authority = Pubkey::try_from(&data[offset..offset + 32])
            .map_err(|_| ProgramError::InvalidAccountData)?;
        offset += 32;

        // verification_level: enum (borsh-serialized)
        let verification_level = if data[offset] == 0 {
            offset += 1;
            let num_signatures = data[offset];
            offset += 1;
            VerificationLevel::Partial { num_signatures }
        } else {
            offset += 1;
            VerificationLevel::Full
        };

        // price_message: PriceFeedMessage
        let feed_id: [u8; 32] = data[offset..offset + 32]
            .try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        offset += 32;

        let price = i64::from_le_bytes(
            data[offset..offset + 8]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?,
        );
        offset += 8;

        let conf = u64::from_le_bytes(
            data[offset..offset + 8]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?,
        );
        offset += 8;

        let exponent = i32::from_le_bytes(
            data[offset..offset + 4]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?,
        );
        offset += 4;

        let publish_time = i64::from_le_bytes(
            data[offset..offset + 8]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?,
        );
        offset += 8;

        let prev_publish_time = i64::from_le_bytes(
            data[offset..offset + 8]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?,
        );
        offset += 8;

        let ema_price = i64::from_le_bytes(
            data[offset..offset + 8]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?,
        );
        offset += 8;

        let ema_conf = u64::from_le_bytes(
            data[offset..offset + 8]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?,
        );
        offset += 8;

        let price_message = PriceFeedMessage {
            feed_id,
            price,
            conf,
            exponent,
            publish_time,
            prev_publish_time,
            ema_price,
            ema_conf,
        };

        // posted_slot: u64
        let posted_slot = u64::from_le_bytes(
            data[offset..offset + 8]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?,
        );

        Ok(PriceUpdateV2 {
            write_authority,
            verification_level,
            price_message,
            posted_slot,
        })
    }

    /// Get price, rejecting if older than `max_age` seconds relative to `clock`.
    /// Equivalent to pyth-solana-receiver-sdk's `get_price_no_older_than`.
    pub fn get_price_no_older_than(
        &self,
        clock: &Clock,
        max_age: u64,
    ) -> std::result::Result<&PriceFeedMessage, crate::errors::RogueTraderError> {
        let age = clock
            .unix_timestamp
            .saturating_sub(self.price_message.publish_time);

        if age < 0 || age as u64 > max_age {
            return Err(crate::errors::RogueTraderError::PythPriceTooStale);
        }

        Ok(&self.price_message)
    }
}
