use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use crate::state::{AgentVault, ClearingHouseState};
use crate::errors::RogueTraderError;

/// Metaplex Token Metadata program ID: metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s
pub const TOKEN_METADATA_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    11, 112, 101, 177, 227, 209, 124, 69, 56, 157, 82, 127, 107, 4, 195, 205,
    88, 184, 108, 115, 26, 160, 253, 181, 73, 182, 209, 188, 3, 248, 41, 70,
]);

#[derive(Accounts)]
#[instruction(bot_id: u8)]
pub struct CreateLpMetadata<'info> {
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

    /// CHECK: Metadata PDA — created by Metaplex program
    #[account(mut)]
    pub metadata: AccountInfo<'info>,

    /// CHECK: LP mint PDA
    #[account(
        seeds = [b"bot_lp_mint", agent_vault.bot_id.to_le_bytes().as_ref()],
        bump = agent_vault.lp_mint_bump,
    )]
    pub lp_mint: AccountInfo<'info>,

    /// CHECK: LP mint authority PDA (signs the CPI)
    #[account(
        seeds = [b"bot_lp_authority", agent_vault.bot_id.to_le_bytes().as_ref()],
        bump = agent_vault.lp_authority_bump,
    )]
    pub lp_authority: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Metaplex Token Metadata program
    #[account(address = TOKEN_METADATA_PROGRAM_ID)]
    pub token_metadata_program: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(
    ctx: Context<CreateLpMetadata>,
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

    // Build CreateMetadataAccountV3 instruction data manually
    // Discriminator: 33
    let mut data = vec![33u8];

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

    // is_mutable: bool = true
    data.push(1);

    // collection_details: Option<CollectionDetails> = None
    data.push(0);

    let accounts = vec![
        AccountMeta::new(*ctx.accounts.metadata.key, false),
        AccountMeta::new_readonly(*ctx.accounts.lp_mint.key, false),
        AccountMeta::new_readonly(*ctx.accounts.lp_authority.key, true), // mint authority, signer
        AccountMeta::new(*ctx.accounts.authority.key, true),             // payer
        AccountMeta::new_readonly(*ctx.accounts.authority.key, false),   // update authority
        AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
        AccountMeta::new_readonly(ctx.accounts.rent.key(), false),
    ];

    let ix = Instruction {
        program_id: TOKEN_METADATA_PROGRAM_ID,
        accounts,
        data,
    };

    // Sign with bot's lp_authority PDA
    let bot_id_bytes = ctx.accounts.agent_vault.bot_id.to_le_bytes();
    let seeds = &[
        b"bot_lp_authority".as_ref(),
        bot_id_bytes.as_ref(),
        &[ctx.accounts.agent_vault.lp_authority_bump],
    ];
    let signer_seeds = &[&seeds[..]];

    invoke_signed(
        &ix,
        &[
            ctx.accounts.metadata.to_account_info(),
            ctx.accounts.lp_mint.to_account_info(),
            ctx.accounts.lp_authority.to_account_info(),
            ctx.accounts.authority.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.rent.to_account_info(),
        ],
        signer_seeds,
    )?;

    Ok(())
}

pub fn borsh_string(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(bytes);
}
