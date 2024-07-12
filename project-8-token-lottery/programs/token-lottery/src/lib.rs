use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{mint_to, Mint, MintTo, TokenAccount, TokenInterface}
};
use switchboard_on_demand::accounts::RandomnessAccountData;
use anchor_spl::metadata::{
    MasterEditionAccount,
    MetadataAccount,
    Metadata,
    CreateMetadataAccountsV3,
    SetAndVerifySizedCollectionItem,
    create_metadata_accounts_v3,
    set_and_verify_sized_collection_item,
    mpl_token_metadata::{
        instructions::{
            CreateMasterEditionV3Cpi, 
            CreateMasterEditionV3CpiAccounts, 
            CreateMasterEditionV3InstructionArgs, 
            CreateMetadataAccountV3Cpi, 
            CreateMetadataAccountV3CpiAccounts, 
            CreateMetadataAccountV3InstructionArgs,
        }, 
        types::{
            Collection, 
            Creator, 
            DataV2,
        },
    }
};

declare_id!("2RTh2Y4e2N421EbSnUYTKdGqDHJH7etxZb3VrWDMpNMY");

#[constant]
pub const NAME: &str = "Token Lottery Ticket";
#[constant]
pub const URI: &str = "Token Lottery";
#[constant]
pub const SYMBOL: &str = "TICKET";


#[program]
pub mod token_lottery {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, start: u64, end: u64, price: u64) -> Result<()> {
        ctx.accounts.token_lottery.bump = ctx.bumps.token_lottery;
        ctx.accounts.token_lottery.mint = ctx.accounts.mint.key();
        ctx.accounts.token_lottery.lottery_start = start;
        ctx.accounts.token_lottery.lottery_end = end;
        ctx.accounts.token_lottery.price = price;
        ctx.accounts.token_lottery.authority = ctx.accounts.signer.key();
        ctx.accounts.token_lottery.randomness_account = Pubkey::default();

        // Is this needed Jacob?
        ctx.accounts.token_lottery.ticket_num = 0;

        // Create Collection Mint

        Ok(())
    }

    pub fn buy_ticket(ctx: Context<BuyTicket>) -> Result<()> {
        let clock = Clock::get()?;

        if clock.slot < ctx.accounts.token_lottery.lottery_start || 
                clock.slot > ctx.accounts.token_lottery.lottery_end {
            return Err(ErrorCode::LotteryNotOpen.into());
        }

        let metadata = &ctx.accounts.metadata.to_account_info();
        let master_edition = &ctx.accounts.master_edition.to_account_info();
        let system_program = &ctx.accounts.system_program.to_account_info();
        let spl_token_program = &ctx.accounts.token_program.to_account_info();
        let spl_metadata_program = &ctx.accounts.token_metadata_program.to_account_info();

        // Take Money
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.payer.to_account_info(),
                    to: ctx.accounts.token_lottery.to_account_info(),
                },
            ),
            ctx.accounts.token_lottery.price,
        )?;

        let signer_seeds: &[&[&[u8]]] = &[&[
            &[ctx.bumps.collection_mint],
        ]];

        // Mint Ticket
        mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.ticket_mint.to_account_info(),
                    to: ctx.accounts.destination.to_account_info(),
                    authority: ctx.accounts.collection_mint.to_account_info(),
                },
                signer_seeds,
            ),
            1,
        )?;

        create_metadata_accounts_v3(
            CpiContext::new_with_signer(
                ctx.accounts.token_metadata_program.to_account_info(),
                CreateMetadataAccountsV3 {
                    metadata: ctx.accounts.metadata.to_account_info(),
                    mint: ctx.accounts.ticket_mint.to_account_info(),
                    mint_authority: ctx.accounts.collection_mint.to_account_info(),
                    update_authority: ctx.accounts.collection_mint.to_account_info(),
                    payer: ctx.accounts.payer.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
                &signer_seeds,
            ),
            DataV2 {
                name: NAME.to_string(),
                symbol: SYMBOL.to_string(),
                uri: URI.to_string(),
                seller_fee_basis_points: 0,
                creators: None,
                collection: None,
                uses: None,
            },
            true,
            true,
            None,
        )?;

        set_and_verify_sized_collection_item(
            CpiContext::new_with_signer(
                ctx.accounts.token_metadata_program.to_account_info(),
                SetAndVerifySizedCollectionItem {
                    metadata: ctx.accounts.metadata.to_account_info(),
                    collection_authority: ctx.accounts.collection_mint.to_account_info(),
                    payer: ctx.accounts.payer.to_account_info(),
                    update_authority: ctx.accounts.collection_mint.to_account_info(),
                    collection_mint: ctx.accounts.collection_mint.to_account_info(),
                    collection_metadata: ctx.accounts.collection_metadata.to_account_info(),
                    collection_master_edition: ctx
                        .accounts
                        .collection_master_edition
                        .to_account_info(),
                },
                &signer_seeds,
            ),
            None,
        )?;

        Ok(())
    }

    pub fn commit_a_winner(ctx: Context<CommitWinner>, randomness_account: Pubkey) -> Result<()> {
        let clock = Clock::get()?;
        let token_lottery = &mut ctx.accounts.token_lottery;

        if clock.slot < token_lottery.lottery_end {
            return Err(ErrorCode::LotteryNotCompleted.into());
        }
        if ctx.accounts.payer.key() != token_lottery.authority {
            return Err(ErrorCode::NotAuthorized.into());
        }

        let randomness_data = RandomnessAccountData::parse(ctx.accounts.randomness_account_data.data.borrow()).unwrap();

        if randomness_data.seed_slot != clock.slot - 1 {
            return Err(ErrorCode::RandomnessAlreadyRevealed.into());
        }

        token_lottery.randomness_account = ctx.accounts.randomness_account_data.key();

        Ok(())
    }

    pub fn choose_a_winner(ctx: Context<ChooseWinner>) -> Result<()> {
        let clock = Clock::get()?;
        let token_lottery = &mut ctx.accounts.token_lottery;

        if ctx.accounts.randomness_account_data.key() != token_lottery.randomness_account {
            return Err(ErrorCode::IncorrectRandomnessAccount.into());
        }
        if ctx.accounts.payer.key() != token_lottery.authority {
            return Err(ErrorCode::NotAuthorized.into());
        }
        if clock.slot < token_lottery.lottery_end {
            return Err(ErrorCode::LotteryNotCompleted.into());
        }

        let randomness_data = 
            RandomnessAccountData::parse(ctx.accounts.randomness_account_data.data.borrow()).unwrap();
        let revealed_random_value = randomness_data.get_value(&clock)
            .map_err(|_| ErrorCode::RandomnessNotResolved)?;

        let randomness_result = 
            revealed_random_value[0] as u32 % token_lottery.ticket_num;

        token_lottery.winner = randomness_result;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct CommitWinner<'info> {
    pub payer: Signer<'info>,

    #[account(mut)]
    pub token_lottery: Account<'info, TokenLottery>,

    /// CHECK: The account's data is validated manually within the handler.
    pub randomness_account_data: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ChooseWinner<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
    pub token_lottery: Account<'info, TokenLottery>,

    /// CHECK: The account's data is validated manually within the handler.
    pub randomness_account_data: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BuyTicket<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
    pub token_lottery: Account<'info, TokenLottery>,

    #[account(
        init,
        payer = payer,
        mint::decimals = 0,
        mint::authority = ticket_mint,
        mint::freeze_authority = ticket_mint,
    )]
    pub ticket_mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = payer,
        associated_token::mint = ticket_mint,
        associated_token::authority = payer
    )]
    pub destination: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    /// CHECK: This account will be initialized by the metaplex program
    pub metadata: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: This account will be initialized by the metaplex program
    pub master_edition: UncheckedAccount<'info>,

    #[account(mut)]
    pub collection_metadata: Account<'info, MetadataAccount>,
    pub collection_master_edition: Account<'info, MasterEditionAccount>,

    #[account(
        mut,
        seeds = [token_lottery.key().as_ref()],
        bump,
    )]
    pub collection_mint: InterfaceAccount<'info, Mint>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub token_metadata_program: Program<'info, Metadata>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init,
        payer = signer,
        space = 8 + TokenLottery::INIT_SPACE,
        // Challenge: Make this be able to run more than 1 lottery at a time
        seeds = [b"token_lottery".as_ref()],
        bump
    )]
    pub token_lottery: Account<'info, TokenLottery>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = signer,
        mint::decimals = 0,
        mint::authority = collection_mint,
        seeds = [token_lottery.key().as_ref()],
        bump,
    )]
    pub collection_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub metadata: Account<'info, MetadataAccount>,

    #[account(mut)]
    pub master_edition: Account<'info, MasterEditionAccount>,

    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = collection_mint,
        associated_token::authority = collection_token_account
    )]
    pub collection_token_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct TokenLottery {
    pub bump: u8,
    pub winner: u32,
    pub lottery_start: u64,
    pub lottery_end: u64,
    // Is it good practice to store SOL on an account used for something else?
    pub lottery_pot_amount: u64,
    pub mint: Pubkey,
    pub ticket_num: u32,
    pub price: u64,
    pub randomness_account: Pubkey,
    pub authority: Pubkey,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Incorrect randomness account")]
    IncorrectRandomnessAccount,
    #[msg("Lottery not completed")]
    LotteryNotCompleted,
    #[msg("Lottery is not open")]
    LotteryNotOpen,
    #[msg("Not authorized")]
    NotAuthorized,
    #[msg("Randomness already revealed")]
    RandomnessAlreadyRevealed,
    #[msg("Randomness not resolved")]
    RandomnessNotResolved,
}
