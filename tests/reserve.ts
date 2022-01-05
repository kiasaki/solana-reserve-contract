//import { expect } from 'chai';
import chai, { expect } from 'chai';
import * as anchor from '@project-serum/anchor';
import { Program } from '@project-serum/anchor';
import * as spl from '@solana/spl-token';
import { Reserve } from '../target/types/reserve';

const { Keypair, PublicKey } = anchor.web3;
const BN = anchor.BN;

anchor.setProvider(anchor.Provider.env());
const program = anchor.workspace.Reserve as Program<Reserve>;
const programId = program.programId;
const wallet = program.provider.wallet;

let daoKeypair = Keypair.generate();
let mintUsdcAuthority = Keypair.generate();
let mintUsdc, tokenUsdcUser, tokenUsdcTreasury;
let mintReserve, mintStaking, tokenTreasuryAccount, tokenReserveDao, tokenReserveUser, tokenStakingUser;
let treasuryKey, treasuryBump, mintReserveKey, mintReserveBump, mintStakingKey, mintStakingBump;
let tokenReserveStakingKey, tokenReserveStakingBump, tokenStakingVestingKey, tokenStakingVestingBump;
let bondKey, bondBump;
let userKey, userBump;

describe('reserve', () => {

  before(async () => {
    mintUsdc = await spl.Token.createMint(
      program.provider.connection,
      wallet.payer,
      mintUsdcAuthority.publicKey,
      null,
      6,
      spl.TOKEN_PROGRAM_ID
    );
    tokenUsdcUser = await mintUsdc.createAccount(wallet.publicKey);
    await mintUsdc.mintTo(
      tokenUsdcUser,
      mintUsdcAuthority,
      [],
      bn(1500, 6).toString()
    );

    // initialize treasury
    const treasuryBaseKey = Keypair.generate().publicKey;
    [treasuryKey, treasuryBump] = await pda(["treasury", treasuryBaseKey]);
    [mintReserveKey, mintReserveBump] = await pda(["treasury_mint_reserve"]);
    [mintStakingKey, mintStakingBump] = await pda(["treasury_mint_staking"]);
    [tokenReserveStakingKey, tokenReserveStakingBump] = await pda(["treasury_token_reserve_staking"]);
    [tokenStakingVestingKey, tokenStakingVestingBump] = await pda(["treasury_token_staking_vesting"]);

    await program.rpc.initialize(
      treasuryBaseKey,
      treasuryBump,
      mintReserveBump,
      mintStakingBump,
      tokenReserveStakingBump,
      tokenStakingVestingBump,
      {
        accounts: {
          signer: wallet.publicKey,
          treasury: treasuryKey,
          mintReserve: mintReserveKey,
          mintStaking: mintStakingKey,
          tokenReserveStaking: tokenReserveStakingKey,
          tokenStakingVesting: tokenStakingVestingKey,
          dao: daoKeypair.publicKey,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
          tokenProgram: spl.TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        }
      }
    );
    tokenUsdcTreasury = await mintUsdc.createAccount(treasuryKey);
    mintReserve = new spl.Token(program.provider.connection, mintReserveKey, spl.TOKEN_PROGRAM_ID, wallet.payer);
    mintStaking = new spl.Token(program.provider.connection, mintStakingKey, spl.TOKEN_PROGRAM_ID, wallet.payer);
    tokenTreasuryAccount = await mintReserve.createAccount(treasuryKey);
    tokenReserveDao = await mintReserve.createAccount(daoKeypair.publicKey);
    tokenReserveUser = await mintReserve.createAccount(wallet.publicKey);
    tokenStakingUser = await mintStaking.createAccount(wallet.publicKey);

    [bondKey, bondBump] = await pda(["bond", treasuryKey, mintUsdc.publicKey]);
    await program.rpc.bondInitialize(bondBump, {
      accounts: {
        signer: wallet.publicKey,
        treasury: treasuryKey,
        bond: bondKey,
        mintBond: mintUsdc.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      }
    });

    [userKey, userBump] = await pda(["user", treasuryKey, wallet.publicKey]);
    await program.rpc.userInitialize(userBump, {
      accounts: {
        signer: wallet.publicKey,
        treasury: treasuryKey,
        user: userKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      }
    });
  });

  it('initialize', async () => {
    let treasuryData = await program.account.treasury.fetch(treasuryKey);
    expect(treasuryData.bump).to.eq(treasuryBump);
    expect(treasuryData.authority).to.eqPubkey(wallet.publicKey);
  });

  it('treasuryConfigure', async () => {
    await program.rpc.treasuryConfigure(
        daoKeypair.publicKey,
        wallet.publicKey,
        bn(100, 0), // staking_rate
        {
          accounts: {
            signer: wallet.publicKey,
            treasury: treasuryKey,
          }
        }
    );
    let treasuryData = await program.account.treasury.fetch(treasuryKey);
    expect(treasuryData.stakingRate).to.eqBN(bn(100, 0));
  });

  it('userInitialize', async () => {
    let userData = await program.account.user.fetch(userKey);
    expect(userData.signer).to.eqPubkey(wallet.publicKey);
  });


  it('bondInitialize', async () => {
    let bondData = await program.account.bond.fetch(bondKey);
    expect(bondData.mintBond).to.eqPubkey(mintUsdc.publicKey);
  });

  it('bondConfigure', async () => {
    await program.rpc.bondConfigure(
      bn(3600, 0), // vesting_period (1 hour)
      bn(1, 9), // rfv_rate
      bn(1, 9), // min_price
      bn(10, 9), // max_payout (1000% of reserves)
      bn(100000, 9), // max_debt (100k RFV)
      bn(5000, 0), // fee (0.5% to dao)
      bn(500, 0), // bcv
      {
        accounts: {
          signer: wallet.publicKey,
          treasury: treasuryKey,
          bond: bondKey,
        }
      }
    );

    let bondData = await program.account.bond.fetch(bondKey);
    expect(bondData.vestingPeriod).to.eqBN(bn(3600, 0));
    expect(bondData.rfvRate).to.eqBN(bn(1, 9));
    expect(bondData.minPrice).to.eqBN(bn(1, 9));
    expect(bondData.maxPayout).to.eqBN(bn(10, 9));
    expect(bondData.maxDebt).to.eqBN(bn(100000, 9));
    expect(bondData.fee).to.eqBN(bn(5000, 0));
  });

  it('bondDeposit', async () => {
    const treasuryData = await program.account.treasury.fetch(treasuryKey);
    const bondData = await program.account.bond.fetch(bondKey);

    await program.rpc.bondDeposit(bn(300, 6), bn(1000000, 9), {
      accounts: {
        signer: wallet.publicKey,
        treasury: treasuryKey,
        bond: bondKey,
        user: userKey,
        mintBond: mintUsdc.publicKey,
        mintReserve: mintReserve.publicKey,
        mintStaking: mintStaking.publicKey,
        tokenBondUser: tokenUsdcUser,
        tokenBondTreasury: tokenUsdcTreasury,
        tokenReserveDao: tokenReserveDao,
        tokenReserveStaking: treasuryData.tokenReserveStaking,
        tokenStakingVesting: treasuryData.tokenStakingVesting,
        tokenProgram: spl.TOKEN_PROGRAM_ID,
      }
    });
    const userData = await program.account.user.fetch(userKey);
    expect(userData.bonds[0].staked.gt(bn(0))).to.be.true;
    //console.log('user bond', userData.bonds[0]);
  });

  it('bondWithdraw', async () => {
    const treasuryData = await program.account.treasury.fetch(treasuryKey);
    await new Promise(resolve => setTimeout(resolve, 2000));
    await program.rpc.bondWithdraw(bn(0), {
      accounts: {
        signer: wallet.publicKey,
        treasury: treasuryKey,
        user: userKey,
        mintStaking: mintStaking.publicKey,
        tokenStakingVesting: treasuryData.tokenStakingVesting,
        tokenStakingUser: tokenStakingUser,
        tokenProgram: spl.TOKEN_PROGRAM_ID,
      }
    });
    const userData = await program.account.user.fetch(userKey);
    expect(userData.bonds[0].claimed.gt(bn(0))).to.be.true;
  });

  it ('stakingWithdraw', async () => {
    await program.rpc.stakingWithdraw(bn(1000, 0), {
      accounts: {
        signer: wallet.publicKey,
        treasury: treasuryKey,
        mintReserve: mintReserve.publicKey,
        mintStaking: mintStaking.publicKey,
        tokenReserveUser: tokenReserveUser,
        tokenReserveStaking: tokenReserveStakingKey,
        tokenStakingUser: tokenStakingUser,
        tokenStakingVesting: tokenStakingVestingKey,
        tokenProgram: spl.TOKEN_PROGRAM_ID,
      }
    });
  });

  it ('stakingDeposit', async () => {
    await program.rpc.stakingDeposit(bn(1000, 0), {
      accounts: {
        signer: wallet.publicKey,
        treasury: treasuryKey,
        mintReserve: mintReserve.publicKey,
        mintStaking: mintStaking.publicKey,
        tokenReserveUser: tokenReserveUser,
        tokenReserveStaking: tokenReserveStakingKey,
        tokenStakingUser: tokenStakingUser,
        tokenProgram: spl.TOKEN_PROGRAM_ID,
      }
    });
  });
});

function bn(value, decimals = 9) {
  return new BN(value).mul(new BN(10).pow(new BN(decimals)));
}

async function pda(seeds, pid = programId) {
  for (let i = 0; i < seeds.length; i++) {
    if (typeof seeds[i] == 'string') {
      seeds[i] = Buffer.from(seeds[i]);
    }
    if (typeof seeds[i].toBuffer == 'function') {
      seeds[i] = seeds[i].toBuffer();
    }
  }
  return await PublicKey.findProgramAddress(seeds, pid);
}

chai.Assertion.addMethod(
  "eqPubkey",
  function (otherIn, message?: string) {
    const self = typeof this._obj === "string" ? new PublicKey(this._obj) : this._obj;
    const other = typeof otherIn === "string" ? new PublicKey(otherIn) : otherIn;
    const msgPrefix = message ? `${message}: ` : "";

    this.assert(
      self.equals(other),
      `${msgPrefix}expected #{this} to equal #{exp} but got #{act}`,
      `${msgPrefix}expected #{this} to not equal #{act}`,
      self.toString(),
      other.toString()
    );
  }
);

chai.Assertion.addMethod(
  "eqBN",
  function (other, message?: string) {
    const msgPrefix = message ? `${message}: ` : "";
    this.assert(
      this._obj.toString() === other.toString(),
      `${msgPrefix}expected #{this} to equal #{exp} but got #{act}`,
      `${msgPrefix}expected #{this} to not equal #{act}`,
      this._obj.toString(),
      other.toString()
    );
  }
);
