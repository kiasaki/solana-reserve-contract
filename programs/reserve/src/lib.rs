use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use std::mem::size_of;

declare_id!("6SMGNVogDVutJ8TpuLkyKUA8aWMbe8xpH5nC9ADw2PXB");

const ONE: u64 = 10_u64.pow(9);

#[error]
pub enum ErrorCode {
    #[msg("unknown error")]
    Unknown,
    #[msg("unauthorized")]
    Unauthorized,
    #[msg("overflow")]
    Overflow,
    #[msg("invalid parameter")]
    InvalidParameter,
    #[msg("bond not configured")]
    BondNotConfigured,
    #[msg("bond at max debt")]
    BondAtMaxDebt,
    #[msg("bond payout too small")]
    BondPayoutTooSmall,
    #[msg("bond payout too big")]
    BondPayoutTooBig,
    #[msg("bond price slipped")]
    BondPriceSlipped,
    #[msg("no space for more bonds")]
    UserNoSpaceForMoreBonds,
}

#[account(zero_copy)]
pub struct Treasury {
    pub key: Pubkey,
    pub bump: u8,
    pub mint_reserve: Pubkey,
    pub mint_staking: Pubkey,
    pub token_reserve_staking: Pubkey,
    pub token_staking_vesting: Pubkey,
    pub dao: Pubkey,
    pub authority: Pubkey,
    // total rfv bonded
    pub total_reserves: u64,
    // 1e9 percent of reserves to mint to stakers per day
    pub staking_rate: u64,
    // last time staking rewards were minted
    pub staking_last: u64,
    _reserved: [u64; 8],
}

#[account(zero_copy)]
pub struct Bond {
    pub bump: u8,
    pub treasury: Pubkey,
    pub mint_bond: Pubkey,
    pub token_decimals: u8,
    pub vesting_period: u64,
    pub rfv_rate: u64,
    pub min_price: u64,
    // 1e9 percent of total reserves
    pub max_payout: u64,
    // max rfv per vesting_period
    pub max_debt: u64,
    // 1e9 percent of payout to mint to dao
    pub fee: u64,
    // bond control variable (determines price / discount)
    pub bcv: u64,
    // rolling amount of rfv accumulated in the past vesting_period
    pub total_debt: u64,
    // last time debt was decayed
    pub total_debt_last: u64,
    // all time rfv bonded
    pub total_debt_alltime: u64,
    _reserved: [u64; 8],
}

#[account(zero_copy)]
pub struct User {
    pub bump: u8,
    pub signer: Pubkey,
    pub treasury: Pubkey,
    pub bonds: [UserBond; 10],
    _reserved: [u64; 8],
}

#[zero_copy]
#[derive(Default)]
pub struct UserBond {
    pub bond: Pubkey,
    // price paid (for display)
    pub price: u64,
    // payout (for display)
    pub payout: u64,
    // staked tokens total
    pub staked: u64,
    // staked tokens claimed
    pub claimed: u64,
    pub vesting_start: u64,
    pub vesting_period: u64,
    _reserved: [u64; 2],
}

#[event]
pub struct EventBondDeposit {
    #[index]
    pub signer: Pubkey,
    #[index]
    pub treasury: Pubkey,
    #[index]
    pub bond: Pubkey,
    #[index]
    pub user: Pubkey,
    pub amount: u64,
    pub price: u64,
    pub payout: u64,
    pub staked: u64,
}

#[event]
pub struct EventBondWithdraw {
    #[index]
    pub signer: Pubkey,
    #[index]
    pub treasury: Pubkey,
    #[index]
    pub bond: Pubkey,
    #[index]
    pub user: Pubkey,
    pub done: bool,
    pub amount: u64,
}

#[event]
pub struct EventStakingDeposit {
    #[index]
    pub signer: Pubkey,
    #[index]
    pub treasury: Pubkey,
    pub staked: u64,
    pub amount: u64,
}

#[event]
pub struct EventStakingWithdraw {
    #[index]
    pub signer: Pubkey,
    #[index]
    pub treasury: Pubkey,
    pub staked: u64,
    pub amount: u64,
}

#[program]
pub mod reserve {
    use super::*;

    #[derive(Accounts)]
    #[instruction(
        key: Pubkey,
        bump: u8,
        mint_reserve_bump: u8,
        mint_staking_bump: u8,
        token_reserve_staking_bump: u8,
        token_staking_vesting_bump: u8
    )]
    pub struct Initialize<'info> {
        #[account(mut)]
        pub signer: Signer<'info>,
        #[account(
            init,
            payer = signer,
            seeds = [b"treasury", key.as_ref()],
            bump = bump,
            space = 8 + size_of::<Treasury>(),
        )]
        pub treasury: AccountLoader<'info, Treasury>,
        #[account(
            init,
            payer = signer,
            seeds = [b"treasury_mint_reserve"],
            bump = mint_reserve_bump,
            owner = token::ID,
            space = Mint::LEN
        )]
        pub mint_reserve: AccountInfo<'info>,
        #[account(
            init,
            payer = signer,
            seeds = [b"treasury_mint_staking"],
            bump = mint_staking_bump,
            owner = token::ID,
            space = Mint::LEN
        )]
        pub mint_staking: AccountInfo<'info>,
        #[account(
            init,
            payer = signer,
            seeds = [b"treasury_token_reserve_staking"],
            bump = token_reserve_staking_bump,
            owner = token::ID,
            space = TokenAccount::LEN
        )]
        pub token_reserve_staking: AccountInfo<'info>,
        #[account(
            init,
            payer = signer,
            seeds = [b"treasury_token_staking_vesting"],
            bump = token_staking_vesting_bump,
            owner = token::ID,
            space = TokenAccount::LEN
        )]
        pub token_staking_vesting: AccountInfo<'info>,
        pub dao: AccountInfo<'info>,
        pub rent: Sysvar<'info, Rent>,
        pub token_program: Program<'info, Token>,
        pub system_program: Program<'info, System>,
    }

    pub fn initialize(
        ctx: Context<Initialize>,
        key: Pubkey,
        bump: u8,
        _mint_reserve_bump: u8,
        _mint_staking_bump: u8,
        _token_reserve_staking_bump: u8,
        _token_staking_vesting_bump: u8,
    ) -> ProgramResult {
        token::initialize_mint(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::InitializeMint {
                    mint: ctx.accounts.mint_reserve.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            ),
            9,
            ctx.accounts.treasury.to_account_info().key,
            Some(ctx.accounts.treasury.to_account_info().key),
        )?;
        token::initialize_mint(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::InitializeMint {
                    mint: ctx.accounts.mint_staking.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            ),
            9,
            ctx.accounts.treasury.to_account_info().key,
            Some(ctx.accounts.treasury.to_account_info().key),
        )?;
        token::initialize_account(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::InitializeAccount {
                account: ctx.accounts.token_reserve_staking.to_account_info(),
                mint: ctx.accounts.mint_reserve.to_account_info(),
                authority: ctx.accounts.treasury.to_account_info(),
                rent: ctx.accounts.rent.to_account_info(),
            },
        ))?;
        token::initialize_account(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::InitializeAccount {
                account: ctx.accounts.token_staking_vesting.to_account_info(),
                mint: ctx.accounts.mint_staking.to_account_info(),
                authority: ctx.accounts.treasury.to_account_info(),
                rent: ctx.accounts.rent.to_account_info(),
            },
        ))?;

        let treasury = &mut ctx.accounts.treasury.load_init()?;
        treasury.key = key;
        treasury.bump = bump;
        treasury.mint_reserve = ctx.accounts.mint_reserve.key();
        treasury.mint_staking = ctx.accounts.mint_staking.key();
        treasury.token_reserve_staking = ctx.accounts.token_reserve_staking.key();
        treasury.token_staking_vesting = ctx.accounts.token_staking_vesting.key();
        treasury.dao = ctx.accounts.dao.key();
        treasury.authority = ctx.accounts.signer.key();
        Ok(())
    }

    #[derive(Accounts)]
    pub struct TreasuryConfigure<'info> {
        #[account(constraint = signer.key() == treasury.load()?.authority @ ErrorCode::Unauthorized)]
        pub signer: Signer<'info>,
        #[account(mut)]
        pub treasury: AccountLoader<'info, Treasury>,
    }

    pub fn treasury_configure(
        ctx: Context<TreasuryConfigure>,
        dao: Pubkey,
        authority: Pubkey,
        staking_rate: u64,
    ) -> ProgramResult {
        let treasury = &mut ctx.accounts.treasury.load_mut()?;
        treasury.dao = dao;
        treasury.authority = authority;
        treasury.staking_rate = staking_rate;
        Ok(())
    }

    #[derive(Accounts)]
    #[instruction(bump: u8)]
    pub struct UserInitialize<'info> {
        pub signer: Signer<'info>,
        pub treasury: AccountLoader<'info, Treasury>,
        #[account(
            init,
            payer = signer,
            seeds = [b"user", treasury.key().as_ref(), signer.key().as_ref()],
            bump = bump,
            space = 8 + size_of::<User>(),
        )]
        pub user: AccountLoader<'info, User>,
        pub system_program: Program<'info, System>,
    }

    pub fn user_initialize(ctx: Context<UserInitialize>, bump: u8) -> ProgramResult {
        let user = &mut ctx.accounts.user.load_init()?;
        user.treasury = ctx.accounts.treasury.key();
        user.signer = ctx.accounts.signer.key();
        user.bump = bump;
        Ok(())
    }

    #[derive(Accounts)]
    #[instruction(bump: u8)]
    pub struct BondInitialize<'info> {
        #[account(constraint = signer.key() == treasury.load()?.authority @ ErrorCode::Unauthorized)]
        pub signer: Signer<'info>,
        pub treasury: AccountLoader<'info, Treasury>,
        #[account(
            init,
            payer = signer,
            seeds = [b"bond", treasury.key().as_ref(), mint_bond.key().as_ref()],
            bump = bump,
            space = 8 + size_of::<Bond>(),
        )]
        pub bond: AccountLoader<'info, Bond>,
        pub mint_bond: Account<'info, Mint>,
        pub system_program: Program<'info, System>,
    }

    pub fn bond_initialize(ctx: Context<BondInitialize>, _bump: u8) -> ProgramResult {
        let bond = &mut ctx.accounts.bond.load_init()?;
        bond.treasury = ctx.accounts.treasury.key();
        bond.mint_bond = ctx.accounts.mint_bond.key();
        bond.token_decimals = ctx.accounts.mint_bond.decimals;
        bond.total_debt_last = unix_now()?;
        Ok(())
    }

    #[derive(Accounts)]
    pub struct BondConfigure<'info> {
        #[account(constraint = signer.key() == treasury.load()?.authority @ ErrorCode::Unauthorized)]
        pub signer: Signer<'info>,
        pub treasury: AccountLoader<'info, Treasury>,
        #[account(mut, has_one = treasury)]
        pub bond: AccountLoader<'info, Bond>,
    }

    pub fn bond_configure(
        ctx: Context<BondConfigure>,
        vesting_period: u64,
        rfv_rate: u64,
        min_price: u64,
        max_payout: u64,
        max_debt: u64,
        fee: u64,
        bcv: u64,
    ) -> ProgramResult {
        let bond = &mut ctx.accounts.bond.load_mut()?;
        require!(vesting_period >= 3600, InvalidParameter);
        bond.vesting_period = vesting_period;
        bond.rfv_rate = rfv_rate;
        bond.min_price = min_price;
        bond.max_payout = max_payout;
        bond.max_debt = max_debt;
        bond.fee = fee;
        if bond.bcv > 0 {
            bond.bcv = bcv;
        }
        Ok(())
    }

    #[derive(Accounts)]
    pub struct BondDeposit<'info> {
        pub signer: Signer<'info>,
        #[account(mut)]
        pub treasury: AccountLoader<'info, Treasury>,
        #[account(mut, has_one = treasury)]
        pub bond: AccountLoader<'info, Bond>,
        #[account(mut, has_one = treasury, has_one = signer)]
        pub user: AccountLoader<'info, User>,
        #[account(constraint = mint_bond.key() == bond.load()?.mint_bond)]
        pub mint_bond: AccountInfo<'info>,
        #[account(mut, constraint = mint_reserve.key() == treasury.load()?.mint_reserve)]
        pub mint_reserve: AccountInfo<'info>,
        #[account(mut, constraint = mint_staking.key() == treasury.load()?.mint_staking)]
        pub mint_staking: Box<Account<'info, Mint>>,
        #[account(
            mut,
            constraint = token_bond_user.mint == bond.load()?.mint_bond,
            constraint = token_bond_user.owner == signer.key(),
        )]
        pub token_bond_user: Box<Account<'info, TokenAccount>>,
        #[account(
            mut,
            constraint = token_bond_treasury.mint == bond.load()?.mint_bond,
            constraint = token_bond_treasury.owner == treasury.key(),
        )]
        pub token_bond_treasury: Box<Account<'info, TokenAccount>>,
        #[account(
            mut,
            constraint = token_reserve_dao.mint == mint_reserve.key(),
            constraint = token_reserve_dao.owner == treasury.load()?.dao,
        )]
        pub token_reserve_dao: Box<Account<'info, TokenAccount>>,
        #[account(
            mut,
            constraint = token_reserve_staking.mint == mint_reserve.key(),
            constraint = token_reserve_staking.owner == treasury.key(),
            constraint = token_reserve_staking.key() == treasury.load()?.token_reserve_staking
        )]
        pub token_reserve_staking: Box<Account<'info, TokenAccount>>,
        #[account(
            mut,
            constraint = token_staking_vesting.mint == mint_staking.key(),
            constraint = token_staking_vesting.owner == treasury.key(),
            constraint = token_staking_vesting.key() == treasury.load()?.token_staking_vesting
        )]
        pub token_staking_vesting: Box<Account<'info, TokenAccount>>,
        pub token_program: Program<'info, Token>,
    }

    pub fn bond_deposit(ctx: Context<BondDeposit>, amount: u64, max_price: u64) -> ProgramResult {
        let now = unix_now()?;
        let key: Pubkey;
        let bump: u8;
        let payout: u64;
        let staked: u64;
        let fee: u64;
        {
            let treasury = &mut ctx.accounts.treasury.load_mut()?;
            let bond = &mut ctx.accounts.bond.load_mut()?;
            let user = &mut ctx.accounts.user.load_mut()?;
            let mint_staking = &ctx.accounts.mint_staking;
            let token_reserve_staking = &ctx.accounts.token_reserve_staking;
            key = treasury.key;
            bump = treasury.bump;
            require!(bond.max_payout != 0, ErrorCode::BondNotConfigured);

            // 1. decay total debt
            let debt_decay = muldiv(
                bond.total_debt,
                now - bond.total_debt_last,
                bond.vesting_period,
            )?;
            bond.total_debt = bond.total_debt.saturating_sub(debt_decay);
            bond.total_debt_last = now;
            require!(bond.total_debt < bond.max_debt, ErrorCode::BondAtMaxDebt);

            // 2. calculate payout
            let debt_ratio = muldiv(bond.total_debt, ONE, treasury.total_reserves.max(1))?;
            let price = (ONE + muldiv(bond.bcv, debt_ratio, ONE)?).max(bond.min_price);
            let amount_scaled = muldiv(amount, ONE, 10_u64.pow(bond.token_decimals as u32))?;
            let value = muldiv(amount_scaled, bond.rfv_rate, ONE)?;
            payout = muldiv(value, ONE, price)?;
            staked = if mint_staking.supply > 0 {
                muldiv(payout, mint_staking.supply, token_reserve_staking.amount)?
            } else {
                payout
            };
            fee = muldiv(payout, bond.fee, ONE)?.min(value.saturating_sub(payout));
            let mut max_payout = muldiv(treasury.total_reserves, bond.max_payout, ONE)?;
            if treasury.total_reserves == 0 {
                max_payout = 1000 * ONE;
            }
            msg!(
                "debtr {} pri {} val {} pay {} fee {} maxpay {}",
                debt_ratio,
                price,
                value,
                payout,
                fee,
                max_payout
            );
            require!(payout > ONE / 100, ErrorCode::BondPayoutTooSmall);
            require!(payout <= max_payout, ErrorCode::BondPayoutTooBig);
            require!(price <= max_price, ErrorCode::BondPriceSlipped);

            // 3. save results
            let mut index = 999;
            for i in 0..user.bonds.len() {
                if user.bonds[i].bond == Pubkey::default() {
                    index = i;
                    break;
                }
            }
            require!(index != 999, ErrorCode::UserNoSpaceForMoreBonds);
            treasury.total_reserves += value;
            bond.total_debt += value;
            bond.total_debt_alltime += value;
            user.bonds[index].bond = ctx.accounts.bond.key();
            user.bonds[index].price = price;
            user.bonds[index].payout = payout;
            user.bonds[index].staked = staked;
            user.bonds[index].claimed = 0;
            user.bonds[index].vesting_start = now;
            user.bonds[index].vesting_period = bond.vesting_period;

            emit!(EventBondDeposit {
                signer: ctx.accounts.signer.key(),
                treasury: ctx.accounts.treasury.key(),
                bond: ctx.accounts.bond.key(),
                user: ctx.accounts.user.key(),
                amount,
                price,
                payout,
                staked,
            });
        }

        // 4. transfers
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.token_bond_user.to_account_info(),
                    to: ctx.accounts.token_bond_treasury.to_account_info(),
                    authority: ctx.accounts.signer.to_account_info(),
                },
            ),
            amount,
        )?;

        if fee > 0 {
            token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    token::MintTo {
                        mint: ctx.accounts.mint_reserve.to_account_info(),
                        to: ctx.accounts.token_reserve_dao.to_account_info(),
                        authority: ctx.accounts.treasury.to_account_info(),
                    },
                    &[&[b"treasury", key.as_ref(), &[bump]]],
                ),
                fee,
            )?;
        }

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.mint_reserve.to_account_info(),
                    to: ctx.accounts.token_reserve_staking.to_account_info(),
                    authority: ctx.accounts.treasury.to_account_info(),
                },
                &[&[b"treasury", key.as_ref(), &[bump]]],
            ),
            payout,
        )?;
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.mint_staking.to_account_info(),
                    to: ctx.accounts.token_staking_vesting.to_account_info(),
                    authority: ctx.accounts.treasury.to_account_info(),
                },
                &[&[b"treasury", key.as_ref(), &[bump]]],
            ),
            staked,
        )?;

        Ok(())
    }

    #[derive(Accounts)]
    pub struct BondWithdraw<'info> {
        pub signer: Signer<'info>,
        pub treasury: AccountLoader<'info, Treasury>,
        #[account(mut, has_one = treasury, has_one = signer)]
        pub user: AccountLoader<'info, User>,
        #[account(constraint = mint_staking.key() == treasury.load()?.mint_staking)]
        pub mint_staking: Box<Account<'info, Mint>>,
        #[account(
            mut,
            constraint = token_staking_vesting.mint == mint_staking.key(),
            constraint = token_staking_vesting.owner == treasury.key(),
            constraint = token_staking_vesting.key() == treasury.load()?.token_staking_vesting,
        )]
        pub token_staking_vesting: Box<Account<'info, TokenAccount>>,
        #[account(
            mut,
            constraint = token_staking_user.mint == mint_staking.key(),
            constraint = token_staking_user.owner == signer.key(),
        )]
        pub token_staking_user: Box<Account<'info, TokenAccount>>,
        pub token_program: Program<'info, Token>,
    }

    pub fn bond_withdraw(ctx: Context<BondWithdraw>, index: u64) -> ProgramResult {
        let now = unix_now()?;
        let key: Pubkey;
        let bump: u8;
        let amount: u64;
        {
            let treasury = &mut ctx.accounts.treasury.load()?;
            let user = &mut ctx.accounts.user.load_mut()?;
            key = treasury.key;
            bump = treasury.bump;

            require!(index < user.bonds.len() as u64, ErrorCode::Overflow);
            require!(
                user.bonds[index as usize].bond != Pubkey::default(),
                ErrorCode::Unknown
            );
            let mut user_bond = &mut user.bonds[index as usize];
            let vesting_progress = muldiv(
                now.saturating_sub(user_bond.vesting_start),
                ONE,
                user_bond.vesting_period,
            )?
            .min(ONE);
            let vested = muldiv(user_bond.staked, vesting_progress, ONE)?;
            amount = vested.saturating_sub(user_bond.claimed);
            user_bond.claimed += amount;
            let bond_key = user_bond.bond.clone();
            let done = user_bond.claimed == user_bond.staked;
            if done {
                user.bonds[index as usize] = UserBond::default();
            }

            emit!(EventBondWithdraw {
                signer: ctx.accounts.signer.key(),
                treasury: ctx.accounts.treasury.key(),
                bond: bond_key,
                user: ctx.accounts.user.key(),
                done,
                amount,
            });
        }

        if amount > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: ctx.accounts.token_staking_vesting.to_account_info(),
                        to: ctx.accounts.token_staking_user.to_account_info(),
                        authority: ctx.accounts.treasury.to_account_info(),
                    },
                    &[&[b"treasury", key.as_ref(), &[bump]]],
                ),
                amount,
            )?;
        }

        Ok(())
    }

    #[derive(Accounts)]
    pub struct StakingDeposit<'info> {
        pub signer: Signer<'info>,
        #[account(mut)]
        pub treasury: AccountLoader<'info, Treasury>,
        #[account(mut, constraint = mint_reserve.key() == treasury.load()?.mint_reserve)]
        pub mint_reserve: Box<Account<'info, Mint>>,
        #[account(mut, constraint = mint_staking.key() == treasury.load()?.mint_staking)]
        pub mint_staking: Box<Account<'info, Mint>>,
        #[account(
            mut,
            constraint = token_reserve_user.mint == mint_reserve.key(),
            constraint = token_reserve_user.owner == signer.key(),
        )]
        pub token_reserve_user: Box<Account<'info, TokenAccount>>,
        #[account(
            mut,
            constraint = token_reserve_staking.mint == mint_reserve.key(),
            constraint = token_reserve_staking.owner == treasury.key(),
            constraint = token_reserve_staking.key() == treasury.load()?.token_reserve_staking,
        )]
        pub token_reserve_staking: Box<Account<'info, TokenAccount>>,
        #[account(
            mut,
            constraint = token_staking_user.mint == mint_staking.key(),
            constraint = token_staking_user.owner == signer.key(),
        )]
        pub token_staking_user: Box<Account<'info, TokenAccount>>,
        pub token_program: Program<'info, Token>,
    }

    pub fn staking_deposit(ctx: Context<StakingDeposit>, amount: u64) -> ProgramResult {
        let now = unix_now()?;
        let key: Pubkey;
        let bump: u8;
        let rebase_amount: u64;
        let value: u64;
        {
            let treasury = &mut ctx.accounts.treasury.load_mut()?;
            key = treasury.key;
            bump = treasury.bump;
            require!(amount > 0, ErrorCode::InvalidParameter);
            let time_elapsed = now.saturating_sub(treasury.staking_last);
            let rate = muldiv(treasury.staking_rate, time_elapsed, 24 * 60 * 60)?;
            rebase_amount = muldiv(ctx.accounts.mint_reserve.supply, rate, ONE)?;
            treasury.staking_last = now;
        }

        if rebase_amount > 0 {
            token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    token::MintTo {
                        mint: ctx.accounts.mint_reserve.to_account_info(),
                        to: ctx.accounts.token_reserve_staking.to_account_info(),
                        authority: ctx.accounts.treasury.to_account_info(),
                    },
                    &[&[b"treasury", key.as_ref(), &[bump]]],
                ),
                amount,
            )?;
            ctx.accounts.token_reserve_staking.reload()?;
        }

        {
            value = if ctx.accounts.mint_staking.supply > 0 {
                muldiv(
                    amount,
                    ctx.accounts.mint_staking.supply,
                    ctx.accounts.token_reserve_staking.amount,
                )?
            } else {
                amount
            };
        }

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.token_reserve_user.to_account_info(),
                    to: ctx.accounts.token_reserve_staking.to_account_info(),
                    authority: ctx.accounts.signer.to_account_info(),
                },
            ),
            amount,
        )?;
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.mint_staking.to_account_info(),
                    to: ctx.accounts.token_staking_user.to_account_info(),
                    authority: ctx.accounts.treasury.to_account_info(),
                },
                &[&[b"treasury", key.as_ref(), &[bump]]],
            ),
            value,
        )?;

        emit!(EventStakingDeposit {
            signer: ctx.accounts.signer.key(),
            treasury: ctx.accounts.treasury.key(),
            staked: value,
            amount,
        });

        Ok(())
    }

    #[derive(Accounts)]
    pub struct StakingWithdraw<'info> {
        pub signer: Signer<'info>,
        #[account(mut)]
        pub treasury: AccountLoader<'info, Treasury>,
        #[account(mut, constraint = mint_reserve.key() == treasury.load()?.mint_reserve)]
        pub mint_reserve: Box<Account<'info, Mint>>,
        #[account(mut, constraint = mint_staking.key() == treasury.load()?.mint_staking)]
        pub mint_staking: Box<Account<'info, Mint>>,
        #[account(
            mut,
            constraint = token_reserve_user.mint == mint_reserve.key(),
            constraint = token_reserve_user.owner == signer.key(),
        )]
        pub token_reserve_user: Box<Account<'info, TokenAccount>>,
        #[account(
            mut,
            constraint = token_reserve_staking.mint == mint_reserve.key(),
            constraint = token_reserve_staking.owner == treasury.key(),
            constraint = token_reserve_staking.key() == treasury.load()?.token_reserve_staking,
        )]
        pub token_reserve_staking: Box<Account<'info, TokenAccount>>,
        #[account(
            mut,
            constraint = token_staking_user.mint == mint_staking.key(),
            constraint = token_staking_user.owner == signer.key(),
        )]
        pub token_staking_user: Box<Account<'info, TokenAccount>>,
        #[account(
            mut,
            constraint = token_staking_vesting.mint == mint_staking.key(),
            constraint = token_staking_vesting.owner == treasury.key(),
            constraint = token_staking_vesting.key() == treasury.load()?.token_staking_vesting,
        )]
        pub token_staking_vesting: Box<Account<'info, TokenAccount>>,
        pub token_program: Program<'info, Token>,
    }

    pub fn staking_withdraw(ctx: Context<StakingWithdraw>, amount: u64) -> ProgramResult {
        let now = unix_now()?;
        let key: Pubkey;
        let bump: u8;
        let rebase_amount: u64;
        let value: u64;
        {
            let treasury = &mut ctx.accounts.treasury.load_mut()?;
            key = treasury.key;
            bump = treasury.bump;
            let time_elapsed = now.saturating_sub(treasury.staking_last);
            let rate = muldiv(treasury.staking_rate, time_elapsed, 24 * 60 * 60)?;
            rebase_amount = muldiv(ctx.accounts.mint_reserve.supply, rate, ONE)?;
            treasury.staking_last = now;
        }

        if rebase_amount > 0 {
            token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    token::MintTo {
                        mint: ctx.accounts.mint_reserve.to_account_info(),
                        to: ctx.accounts.token_reserve_staking.to_account_info(),
                        authority: ctx.accounts.treasury.to_account_info(),
                    },
                    &[&[b"treasury", key.as_ref(), &[bump]]],
                ),
                amount,
            )?;
            ctx.accounts.token_reserve_staking.reload()?;
        }

        {
            value = muldiv(
                amount,
                ctx.accounts.token_reserve_staking.amount,
                ctx.accounts.mint_staking.supply,
            )?;
        }

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.token_staking_user.to_account_info(),
                    to: ctx.accounts.token_staking_vesting.to_account_info(),
                    authority: ctx.accounts.signer.to_account_info(),
                },
            ),
            amount,
        )?;
        token::burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Burn {
                    mint: ctx.accounts.mint_staking.to_account_info(),
                    to: ctx.accounts.token_staking_vesting.to_account_info(),
                    authority: ctx.accounts.treasury.to_account_info(),
                },
                &[&[b"treasury", key.as_ref(), &[bump]]],
            ),
            amount,
        )?;
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.token_reserve_staking.to_account_info(),
                    to: ctx.accounts.token_reserve_user.to_account_info(),
                    authority: ctx.accounts.treasury.to_account_info(),
                },
                &[&[b"treasury", key.as_ref(), &[bump]]],
            ),
            value,
        )?;

        emit!(EventStakingWithdraw {
            signer: ctx.accounts.signer.key(),
            treasury: ctx.accounts.treasury.key(),
            staked: amount,
            amount: value,
        });

        Ok(())
    }
}

fn muldiv(a: u64, m: u64, d: u64) -> Result<u64> {
    msg!("muldiv a {} m {} d {}", a, m, d);
    let result = a as u128 * m as u128 / d as u128;
    if result > u64::MAX as u128 {
        return Err(ErrorCode::Overflow.into());
    }
    Ok(result as u64)
}

fn unix_now() -> Result<u64> {
    Ok(Clock::get()?.unix_timestamp as u64)
}
