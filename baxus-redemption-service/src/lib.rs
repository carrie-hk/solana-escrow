use anchor_lang::{prelude::*, solana_program::entrypoint_deprecated::ProgramResult};
use anchor_spl::token::{TokenAccount, Token, Mint};

// You must be sure to update declare_id to match the actual runtime ID
declare_id!("AuRbLaNg1BnPbu9d9sNM6hVTLAnyNBZVkdHCWXX14csw");

// On the Solana side of things, the BAXUS redemption service will consist of transferring an existing token account's NFT to a BAXUS controlled escrow account,
// where it will be held while the physical asset is shipped to the physical owner
// The BAXUS escrow account will be created for this transaction and will live at a PDA - the customer will fund the creation of this account
//
// When the physical asset has been delivered and signed for by the physical owner, the NFT will be burned (this is an existing function in the SPL Token 
// library, and therefore that functionality probably doesn't need to be created here) and the BAXUS escrow account used to hold it will be closed (again,
// this can make use of preexisting SPL functionality by just transferring all of the rent money to a permanent BAXUS account)
//
// We will define three capabilities in this program:
// 1) Initialize Redemption - transfer the customer's NFT into a BAXUS escrow account, and store information about the redemption process in a reusable account
// 2) Return Asset Token    - in the event that the Know Your Customer process prevents BAXUS from being able to physically transfer custody of the asset to the customer, return the 
//                            token to the customer's token account and close the escrow and redemption info accounts
// 3) Burn Asset Token      - if the customer verifies identity and the asset is delivered to them, the asset token is burned and the escrow and redemption info accounts are closed
//
// The existing token account will be called customer_token_account
// The customer account used to fund the escrow account will be called customer_payment_account
// The BAXUS escrow account will be called baxus_escrow_account

#[program]
pub mod baxus_redemption_service {

    use super::*;
    pub fn initialize_redemption(ctx: Context<InitializeRedemption>) -> ProgramResult {
        let redemption_info = &mut ctx.accounts.redemption_info;
        redemption_info.customer_token_account = ctx.accounts.customer_token_account.key();
        redemption_info.customer_payment_account = ctx.accounts.customer_payment_account.key();
        redemption_info.escrow_bump = *ctx.bumps.get("baxus_escrow_account").unwrap();
        redemption_info.redemption_bump = *ctx.bumps.get("redemption_info").unwrap();

        anchor_spl::token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(), 
                anchor_spl::token::Transfer {
                    from: ctx.accounts.customer_token_account.to_account_info(),
                    to: ctx.accounts.baxus_escrow_account.to_account_info(),
                    authority: ctx.accounts.customer_payment_account.to_account_info(),
                }), 
            1,
        )?;

        Ok(())
    }
    
    pub fn return_asset_token(ctx: Context<ReturnAssetToken>) -> ProgramResult {

        anchor_spl::token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(), 
                anchor_spl::token::Transfer {
                    from: ctx.accounts.baxus_escrow_account.to_account_info(),
                    to: ctx.accounts.customer_token_account.to_account_info(),
                    authority: ctx.accounts.baxus_escrow_account.to_account_info()
                }, 
                &[&[
                    ctx.accounts.token_mint_account.key().as_ref(), 
                    &[ctx.accounts.redemption_info.escrow_bump],
                ]]
            ), 
            1)?;

        anchor_spl::token::close_account(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(), 
                anchor_spl::token::CloseAccount {
                    account: ctx.accounts.baxus_escrow_account.to_account_info(),
                    destination: ctx.accounts.customer_payment_account.to_account_info(),
                    authority: ctx.accounts.baxus_escrow_account.to_account_info(),
                }, 
                &[&[
                    ctx.accounts.token_mint_account.key().as_ref(), 
                    &[ctx.accounts.redemption_info.escrow_bump],
                ]]
            ),
        )?;

        Ok(())
    }

    pub fn burn_asset_token(ctx: Context<BurnAssetToken>) -> ProgramResult{

        anchor_spl::token::burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(), 
                anchor_spl::token::Burn {
                    mint: ctx.accounts.token_mint_account.to_account_info(),
                    to: ctx.accounts.baxus_escrow_account.to_account_info(),
                    authority: ctx.accounts.baxus_escrow_account.to_account_info(),
                }, 
                &[&[
                    ctx.accounts.token_mint_account.key().as_ref(), 
                    &[ctx.accounts.redemption_info.escrow_bump],
                ]]
            ), 
            1)?;

        // Add anchor_spl::token::close() instruction, since you can't use the close attribute in the baxus_escrow_account account
        anchor_spl::token::close_account(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(), 
                anchor_spl::token::CloseAccount {
                    account: ctx.accounts.baxus_escrow_account.to_account_info(),
                    destination: ctx.accounts.customer_payment_account.to_account_info(),
                    authority: ctx.accounts.baxus_escrow_account.to_account_info(),
                }, 
                &[&[
                    ctx.accounts.token_mint_account.key().as_ref(), 
                    &[ctx.accounts.redemption_info.escrow_bump],
                ]]
            ),
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
// Anchor requires an underscore prefix for any variable name that isn't used in a function
#[instruction()]
pub struct InitializeRedemption<'info> {
    #[account(
        init, 
        payer = customer_payment_account, 
        // We will initialize the redemption_info account to live at a PDA, and we will need to store the bump so that when we call return or burn, we make sure we're using the correct redemption_info
        seeds = [token_mint_account.key().as_ref(), b"redemption".as_ref()],
        bump,
        // Allocate double the space we currently need in case we need to re-deploy with more fields in RedemptionInfo (Solana might allow you to dynamically resize on 
        // re-deploy, but who knows)
        // TO DO: Discuss costs of doing that, whether or not we want more than 2* the necessary space, etc etc
        space = 8 + 2*(32 + 32 + 1 + 1))
    ]
    pub redemption_info: Account<'info, RedemptionInfo>,

    #[account(mut, constraint = customer_token_account.mint == token_mint_account.key())]
    pub customer_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub customer_payment_account: Signer<'info>,

    // We will need to provide the account containing the NFT's mint for the creation of the baxus_escrow_account
    pub token_mint_account: Account<'info, Mint>,

    #[account(
        init, 
        payer = customer_payment_account, 
        // TO DO: Make sure we are using meaningful/scalable seeds and bump
        seeds = [token_mint_account.key().as_ref()], 
        bump, 
        token::mint = token_mint_account,
        token::authority = baxus_escrow_account)
    ]
    pub baxus_escrow_account: Account<'info, TokenAccount>,

    // Include a Token Program account because we need to ask it transfer the NFT from the customer_token_account to the baxus_escrow_account
    pub token_program: Program<'info, Token>,

    // The Token Program requires that we include a Rent Sysvar account
    pub rent: Sysvar<'info, Rent>,

    // Include a System Program account because we need it in order to create baxus_escrow_account
    pub system_program: Program<'info, System>
}

#[derive(Accounts)]
pub struct ReturnAssetToken<'info> {
    #[account(
        mut,
        seeds = [token_mint_account.key().as_ref(), b"redemption".as_ref()],
        bump = redemption_info.redemption_bump,
        close = customer_payment_account)
    ]
    pub redemption_info: Account<'info, RedemptionInfo>,

    // The customer_token_account must be mutable in order for it to accept the token
    #[account(
        mut, 
        constraint = customer_token_account.owner == *customer_payment_account.key,
        constraint = redemption_info.customer_token_account == customer_token_account.key())
    ]
    pub customer_token_account: Account<'info, TokenAccount>,

    #[account(constraint = redemption_info.customer_payment_account == customer_payment_account.key())] 
    pub customer_payment_account: SystemAccount<'info>,

    #[account(mut)]
    pub token_mint_account: Account<'info, Mint>,

    #[account(
        mut,
        // TO DO: Confirm that we are okay using the mint as a seed, which implies that there will only ever be one token for a given mint
        seeds = [token_mint_account.key().as_ref()], 
        bump = redemption_info.escrow_bump)
    ]
    pub baxus_escrow_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct BurnAssetToken<'info> {
    #[account(
        mut,
        seeds = [token_mint_account.key().as_ref(), b"redemption".as_ref()],
        bump = redemption_info.redemption_bump,
        // After the asset token is burned, we can close the RedemptionInfo account and send its rent back to the customer
        close = customer_payment_account)
    ]
    pub redemption_info: Account<'info, RedemptionInfo>,

    // Include customer_token_account so we can properly constrain the redemption_info account, and make sure it is associated with the correct customer_payment_account
    #[account(
        constraint = customer_token_account.owner == *customer_payment_account.key,
        constraint = redemption_info.customer_token_account == customer_token_account.key())
    ]
    pub customer_token_account: Account<'info, TokenAccount>,

    #[account(constraint = redemption_info.customer_payment_account == customer_payment_account.key())]
    pub customer_payment_account: SystemAccount<'info>,

    #[account(mut)]
    pub token_mint_account: Account<'info, Mint>,

    #[account(
        mut,
        // TO DO: Confirm that we are okay using the mint as a seed, which implies that there will only ever be one token for a given mint
        seeds = [token_mint_account.key().as_ref()], 
        bump = redemption_info.escrow_bump)
    ]
    pub baxus_escrow_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[account]
pub struct RedemptionInfo {
    customer_token_account: Pubkey,
    customer_payment_account: Pubkey,
    escrow_bump: u8,
    redemption_bump: u8,
}
