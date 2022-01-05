## solana reserve protocol

_similar to OlympusDAO but without most bells and whistles_

### introduction

A reserve protocol issues a token that is backed by the value of its treasury.

Participants get to mint the reserve asset by exchanging an asset the treasury wants for it. It’ll usually be exchangeable at a rate that is close to the current market price of the token. The exchange rate goes down over time and jumps back up every time somebody mints some. Usually, the payout is vested linearly over some amount of time.

Participants also get to stake their reserve tokens for a staked reserve token. Over time, the protocol distributes the profit it makes from people minting (the minting exchange rate is always higher than the amount of reserve tokens paid out) to people that staked. This is controlled by a configured percentage of total risk free value accumulated to distribute per day. This means that, if people stop minting, there will be a point where there is not more profit to distribute. But if minting keeps happening, there will be rewards to give to staking participants.

### developing

The smart contracts are built using project serum's Anchor framework.

All you need should need is `anchor test` to develop.

### deploying

Deploying to devnet for the first time:

```
solana-keygen new -o ~/owner.json
solana-keygen new -o ~/program.json
solana airdrop 2 -u d -k ~/owner.json
anchor build # update "declare_id!" to match ~/program.json
solana program deploy -u d -k ~/owner.json --program-id ~/program.json target/deploy/reserve.so
```

### license

MIT
