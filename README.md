# escrow

Solana escrow program built with Anchor. A maker locks token A and asks for a fixed amount of token B. A taker fills it in full, or the maker refunds.

## Accounts

* `Escrow` PDA, seeds `["escrow", maker, seed]`: holds `seed`, `maker`, `mint_a`, `mint_b`, `receive`, `bump`.
* Vault: associated token account for mint A, authority is the escrow PDA.

## Instructions

* `make(seed, receive, deposit)`: creates the escrow and vault, deposits `deposit` of token A.
* `take()`: taker pays `receive` of token B to the maker, receives the vault balance, escrow and vault close.
* `refund()`: maker gets the vault balance back, escrow and vault close.
* `update(new_receive)`: maker changes the price while the escrow is open.

## Build

```
anchor build
anchor test
```
