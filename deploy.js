const spl = require("@solana/spl-token");
const anchor = require("@project-serum/anchor");
const { Program } = anchor;
const { Keypair, PublicKey } = anchor.web3;
const BN = anchor.BN;

(async function () {
  const wallet = new anchor.Wallet(
    Keypair.fromSecretKey(
      Uint8Array.from(require(process.env.HOME + "/owner.json"))
    )
  );
  const connection = new anchor.web3.Connection(
    "https://api.devnet.solana.com",
    "recent"
  );
  anchor.setProvider(new anchor.Provider(connection, wallet, {}));
  const idl = JSON.parse(
    require("fs").readFileSync("./target/idl/reserve.json", "utf-8")
  );

  const programId = new PublicKey(
    "6SMGNVogDVutJ8TpuLkyKUA8aWMbe8xpH5nC9ADw2PXB"
  );
  const mintUsdcKey = new PublicKey(
    "Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr"
  );
  const daoKey = new PublicKey(wallet.publicKey);
  const treasuryBaseKey = Keypair.generate().publicKey;
  const [treasuryKey, treasuryBump] = await pda(programId, [
    "treasury",
    treasuryBaseKey,
  ]);
  const [bondKey, bondBump] = await pda(programId, [
    "bond",
    treasuryKey,
    mintUsdcKey,
  ]);
  const [mintReserveKey, mintReserveBump] = await pda(programId, [
    "treasury_mint_reserve",
  ]);
  const [mintStakingKey, mintStakingBump] = await pda(programId, [
    "treasury_mint_staking",
  ]);
  const program = new anchor.Program(idl, programId);
  const mintUsdc = new spl.Token(
    program.provider.connection,
    mintUsdcKey,
    spl.TOKEN_PROGRAM_ID,
    wallet.payer
  );
  const mintReserve = new spl.Token(
    program.provider.connection,
    mintReserveKey,
    spl.TOKEN_PROGRAM_ID,
    wallet.payer
  );
  const mintStaking = new spl.Token(
    program.provider.connection,
    mintStakingKey,
    spl.TOKEN_PROGRAM_ID,
    wallet.payer
  );

  // 1. Create the treasury singleton
  const [tokenReserveStakingKey, tokenReserveStakingBump] = await pda(
    programId,
    ["treasury_token_reserve_staking"]
  );
  const [tokenStakingVestingKey, tokenStakingVestingBump] = await pda(
    programId,
    ["treasury_token_staking_vesting"]
  );
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
        tokenStakingTmp: tokenStakingTmpKey,
        dao: daoKey,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        tokenProgram: spl.TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      },
    }
  );
  const treasuryData = await program.account.treasury.fetch(treasuryKey);
  console.log("treasuryKey", treasuryKey.toString());
  console.log("mintReserve", treasuryData.mintReserve.toString());
  console.log("mintStaking", treasuryData.mintStaking.toString());

  // 2. initialize bond
  await program.rpc.bondInitialize(bondBump, {
    accounts: {
      signer: wallet.publicKey,
      treasury: treasuryKey,
      bond: bondKey,
      mintBond: mintUsdcKey,
      systemProgram: anchor.web3.SystemProgram.programId,
    },
  });
  console.log("bondKey", bondKey.toString());

  // 3. configure bond
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
      },
    }
  );

  // 5. create dao reserve token account & treasury bond token account
  const tokenReserveDaoKey = await spl.Token.getAssociatedTokenAddress(
    spl.ASSOCIATED_TOKEN_PROGRAM_ID,
    spl.TOKEN_PROGRAM_ID,
    mintReserve.publicKey,
    daoKey,
    true
  );
  await mintReserve.createAssociatedTokenAccountInternal(
    daoKey,
    tokenReserveDaoKey
  );
  console.log("tokenReserveDaoKey", tokenReserveDaoKey.toString());
  const tokenUsdcTreasuryKey = await spl.Token.getAssociatedTokenAddress(
    spl.ASSOCIATED_TOKEN_PROGRAM_ID,
    spl.TOKEN_PROGRAM_ID,
    mintUsdc.publicKey,
    treasuryKey,
    true
  );
  await mintUsdc.createAssociatedTokenAccountInternal(
    treasuryKey,
    tokenUsdcTreasuryKey
  );
  console.log("tokenUsdcTreasuryKey", tokenUsdcTreasuryKey.toString());

  process.exit(0);
})();

function bn(value, decimals = 9) {
  return new BN(value).mul(new BN(10).pow(new BN(decimals)));
}

async function pda(seeds, pid = programId) {
  for (let i = 0; i < seeds.length; i++) {
    if (typeof seeds[i] == "string") {
      seeds[i] = Buffer.from(seeds[i]);
    }
    if (typeof seeds[i].toBuffer == "function") {
      seeds[i] = seeds[i].toBuffer();
    }
  }
  return await PublicKey.findProgramAddress(seeds, pid);
}
