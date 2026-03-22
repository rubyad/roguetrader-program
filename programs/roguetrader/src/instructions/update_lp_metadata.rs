use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use crate::state::{AgentVault, ClearingHouseState};
use crate::errors::RogueTraderError;
use crate::instructions::create_lp_metadata::{TOKEN_METADATA_PROGRAM_ID, borsh_string};

#[derive(Accounts)]
#[instruction(bot_id: u8)]
pub struct UpdateLpMetadata<'info> {
    #[account(
        seeds = [b"clearing_house"],
        bump = clearing_house.bump,
        has_one = authority,
    )]
    pub clearing_house: Account<'info, ClearingHouseState>,

    #[account(
        seeds = [b"agent_vault", agent_vault.bot_id.to_le_bytes().as_ref()],
        bump = agent_vault.bump,
    )]
    pub agent_vault: Account<'info, AgentVault>,

    /// CHECK: Metadata PDA — updated by Metaplex program
    #[account(mut)]
    pub metadata: AccountInfo<'info>,

    /// Authority is the update authority (set during create_lp_metadata)
    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Metaplex Token Metadata program
    #[account(address = TOKEN_METADATA_PROGRAM_ID)]
    pub token_metadata_program: AccountInfo<'info>,
}

pub fn handler(
    ctx: Context<UpdateLpMetadata>,
    bot_id: u8,
    name: String,
    symbol: String,
    uri: String,
) -> Result<()> {
    // Validate bot_id matches the agent_vault
    require!(
        ctx.accounts.agent_vault.bot_id == bot_id,
        RogueTraderError::InvalidBotId
    );

    // Validate string lengths
    require!(name.len() <= 32, RogueTraderError::InvalidConfig);
    require!(symbol.len() <= 10, RogueTraderError::InvalidConfig);
    require!(uri.len() <= 200, RogueTraderError::InvalidConfig);

    // Build UpdateMetadataAccountV2 instruction data manually
    // Discriminator: 15
    let mut data = vec![15u8];

    // Option<DataV2> = Some(...)
    data.push(1);
    // DataV2 struct (borsh-serialized):
    // name: String
    borsh_string(&mut data, &name);
    // symbol: String
    borsh_string(&mut data, &symbol);
    // uri: String
    borsh_string(&mut data, &uri);
    // seller_fee_basis_points: u16
    data.extend_from_slice(&0u16.to_le_bytes());
    // creators: Option<Vec<Creator>> = None
    data.push(0);
    // collection: Option<Collection> = None
    data.push(0);
    // uses: Option<Uses> = None
    data.push(0);

    // new_update_authority: Option<Pubkey> = None (keep current)
    data.push(0);

    // primary_sale_happened: Option<bool> = None (keep current)
    data.push(0);

    // is_mutable: Option<bool> = Some(true)
    data.push(1);
    data.push(1);

    let accounts = vec![
        AccountMeta::new(*ctx.accounts.metadata.key, false),
        AccountMeta::new_readonly(*ctx.accounts.authority.key, true), // update authority, signer
    ];

    let ix = Instruction {
        program_id: TOKEN_METADATA_PROGRAM_ID,
        accounts,
        data,
    };

    // Authority (deployer) is the update authority and signs directly — no PDA signing needed
    invoke(
        &ix,
        &[
            ctx.accounts.metadata.to_account_info(),
            ctx.accounts.authority.to_account_info(),
        ],
    )?;

    Ok(())
}
