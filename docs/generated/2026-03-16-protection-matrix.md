# Operation Protection Matrix

Coverage of password and biometric protection across every operation in Sage that can reach a wallet
secret.

**Revised 2026-08-22** for the password gate. The previous revision described enforcement as a
frontend responsibility split between `ConfirmationDialog` and per-call-site `requestPassword` calls.
Neither exists any more — see
[2026-08-21-password-gate-design.md](2026-08-21-password-gate-design.md).

## Architecture

Rust decides when authentication is required and prompts for it. No caller supplies a password.

`crates/sage-api/password-gating.json` assigns every password-bearing endpoint one of three modes,
and the endpoint macro expands the gate accordingly:

| Mode          | Gate behaviour                                                                |
| ------------- | ----------------------------------------------------------------------------- |
| `always`      | prompt on every call, against the active wallet                               |
| `fingerprint` | prompt on every call, against `req.fingerprint` rather than the active wallet |
| `auto_submit` | prompt only when `req.auto_submit` is set; otherwise clear `req.password`     |

`auto_submit` mode exists because those endpoints only build a transaction until the caller asks for
it to be submitted. This is what decides **where the prompt lands**:

- **Normal UI flows** call the endpoint with `auto_submit` unset, get unsigned coin spends back, and
  render `ConfirmationDialog`. Nothing is asked for during the build. The dialog's Submit button
  calls `sign_coin_spends`, which is `always`, so exactly one prompt appears — after Submit.
  (If the user pressed Sign first, Submit reuses that signature and does not prompt again.)
- **WalletConnect handlers** set `auto_submit: true`, so signing happens inside the command and the
  prompt lands there.
- **App bridge requests** do not go through the endpoint macro. They have their own gate in
  `process_after_approval`, which runs after the user approves the request summary and before the
  handler executes. It is unconditional for the four approval bodies that reach a secret.

Password and biometric remain mutually exclusive. `PasswordContext` now answers a Rust
`PasswordRequest` event instead of being called, but its three-way decision — password dialog,
biometric gate with a 5-minute cache, or `NoAuthNeeded` — is unchanged. A wallet with a password set
never triggers biometric.

## Matrix — UI operations

Legend: ✅ = reaches a secret and is gated, ❌ = reaches no secret, no gate needed

| Operation                         | Endpoint                    | Mode          | Prompt appears     | Call site                               |
| --------------------------------- | --------------------------- | ------------- | ------------------ | --------------------------------------- |
| **Transactions**                  |                             |               |                    |                                         |
| Send XCH                          | `send_xch`                  | `auto_submit` | ConfirmationDialog | `Send.tsx`                              |
| Send CAT                          | `send_cat`                  | `auto_submit` | ConfirmationDialog | `Send.tsx`                              |
| Bulk send XCH                     | `bulk_send_xch`             | `auto_submit` | ConfirmationDialog | `Send.tsx`                              |
| Bulk send CAT                     | `bulk_send_cat`             | `auto_submit` | ConfirmationDialog | `Send.tsx`                              |
| Combine coins                     | `combine`                   | `auto_submit` | ConfirmationDialog | `OwnedCoinsCard.tsx`                    |
| Split coins                       | `split`                     | `auto_submit` | ConfirmationDialog | `OwnedCoinsCard.tsx`                    |
| Auto-combine XCH / CAT            | `auto_combine_xch` / `_cat` | `auto_submit` | ConfirmationDialog | `OwnedCoinsCard.tsx`                    |
| Issue CAT                         | `issue_cat`                 | `auto_submit` | ConfirmationDialog | `IssueToken.tsx`                        |
| Multi-send                        | `multi_send`                | `auto_submit` | —                  | No frontend binding                     |
| Action system                     | `create_transaction`        | `auto_submit` | ConfirmationDialog | No frontend binding                     |
| Sign coin spends (Sign button)    | `sign_coin_spends`          | `always`      | immediately        | `ConfirmationDialog.tsx`                |
| Sign coin spends (Submit button)  | `sign_coin_spends`          | `always`      | immediately        | `ConfirmationDialog.tsx`                |
| **NFTs / DIDs**                   |                             |               |                    |                                         |
| Bulk mint NFTs                    | `bulk_mint_nfts`            | `auto_submit` | ConfirmationDialog | `MintNft.tsx`                           |
| Transfer NFTs                     | `transfer_nfts`             | `auto_submit` | ConfirmationDialog | `MultiSelectActions.tsx`                |
| Burn NFTs                         | `transfer_nfts`             | `auto_submit` | ConfirmationDialog | `MultiSelectActions.tsx`                |
| Add NFT URI                       | `add_nft_uri`               | `auto_submit` | ConfirmationDialog | `NftCard.tsx`                           |
| Assign NFTs to DID                | `assign_nfts_to_did`        | `auto_submit` | ConfirmationDialog | `NftCard.tsx`, `MultiSelectActions.tsx` |
| Create DID                        | `create_did`                | `auto_submit` | ConfirmationDialog | `CreateProfile.tsx`                     |
| Transfer / burn DIDs              | `transfer_dids`             | `auto_submit` | ConfirmationDialog | `DidList.tsx`                           |
| Normalize DIDs                    | `normalize_dids`            | `auto_submit` | ConfirmationDialog | `DidList.tsx`                           |
| **Options**                       |                             |               |                    |                                         |
| Mint option                       | `mint_option`               | `auto_submit` | ConfirmationDialog | `MintOption.tsx`                        |
| Transfer / burn options           | `transfer_options`          | `auto_submit` | ConfirmationDialog | `useOptionActions.tsx`                  |
| Exercise options                  | `exercise_options`          | `auto_submit` | ConfirmationDialog | `useOptionActions.tsx`                  |
| **Clawback**                      |                             |               |                    |                                         |
| Claw back coins                   | `combine`                   | `auto_submit` | ConfirmationDialog | `ClawbackCoinsCard.tsx`                 |
| Finalize clawback                 | `finalize_clawback`         | `auto_submit` | ConfirmationDialog | `ClawbackCoinsCard.tsx`                 |
| **Offers**                        |                             |               |                    |                                         |
| Make offer                        | `make_offer`                | `always`      | immediately        | `useOfferProcessor.ts`                  |
| Take offer                        | `take_offer`                | `always`      | immediately        | `Offer.tsx`                             |
| Cancel offer                      | `cancel_offer`              | `auto_submit` | ConfirmationDialog | `OfferRowCard.tsx`                      |
| Cancel all offers                 | `cancel_offers`             | `auto_submit` | ConfirmationDialog | `Offers.tsx`                            |
| **Secrets / key management**      |                             |               |                    |                                         |
| View mnemonic / secret key        | `get_secret_key`            | `fingerprint` | immediately        | `WalletCard.tsx`                        |
| Delete wallet key                 | `delete_key`                | `fingerprint` | immediately        | `WalletCard.tsx`                        |
| Import key                        | `import_key`                | ❌ ungated    | —                  | `CreateWallet.tsx`, `ImportWallet.tsx`  |
| Set / change / remove password    | `change_password`           | ❌ ungated    | inline form        | `PasswordManagementDialog.tsx`          |
| **Key derivation**                |                             |               |                    |                                         |
| Increase derivation (hardened)    | `increase_derivation_index` | `always`      | immediately        | `Settings.tsx`                          |
| Increase derivation (unhardened)  | `increase_derivation_index` | `always`      | immediately        | `Settings.tsx`                          |
| **Ungated by design**             |                             |               |                    |                                         |
| Start RPC server, run-on-startup  | —                           | ❌            | `requireLocalAuth` | `Settings.tsx`                          |
| Submit pre-signed transaction     | `submit_transaction`        | ❌            | —                  | Operates on a signed bundle             |
| View coin spends                  | `view_coin_spends`          | ❌            | —                  | Read-only                               |
| Balances, addresses, NFTs         | —                           | ❌            | —                  | Read-only                               |
| Login / logout                    | —                           | ❌            | —                  | No secret access                        |
| Rename / resync / emoji           | —                           | ❌            | —                  | Metadata only                           |
| Enable / disable biometric toggle | —                           | ❌            | —                  | No-op on protected wallets              |

Notes:

- **`import_key` is genuinely ungated.** `ImportKey` has no `password` field; keys are imported
  encrypted with the `b""` sentinel and protected afterwards through Settings.
- **`change_password` is deliberately ungated.** Its `old_password` is management form data, not a
  wallet unlock, and the form collects it directly.
- **`increase_derivation_index` over-prompts on the unhardened path.** Its `extract_secrets` call is
  inside `if hardened` (`crates/sage/src/endpoints/actions.rs:204`), so an unhardened-only call
  reaches no secret — but the mode is per-endpoint, so the gate prompts anyway. `Settings.tsx` calls
  it with `hardened: false` whenever the wallet has no secrets or hardened derivation is off, and a
  protected wallet gets a password dialog for a request that will not use the answer. Before the
  password gate the frontend suppressed this itself with `has_secrets && hardened`. Expressing it
  would need a fourth gating mode conditioned on `req.hardened`; the same shape as `auto_submit`.
- **Clawback and auto-combine reuse `combine`**, which is why they inherit its mode.
- Two endpoints carry an `auto_submit` field but are `always`: `sign_coin_spends` and `take_offer`
  both reach the keychain before consulting it. A drift test enforces this against the
  implementations rather than against the field's presence.

## Matrix — WalletConnect operations

WC handlers set `auto_submit: true` (except `chip0002_signCoinSpends`), so the gate fires inside the
command. `HandlerContext` no longer carries `requestPassword` or `hasPassword`.

| Operation                             | Endpoint                       | `auto_submit` | Mode          | Prompts |
| ------------------------------------- | ------------------------------ | :-----------: | ------------- | :-----: |
| `chia_send` (XCH / CAT)               | `send_xch` / `send_cat`        |    `true`     | `auto_submit` |   ✅    |
| `chia_bulkMintNfts`                   | `bulk_mint_nfts`               |    `true`     | `auto_submit` |   ✅    |
| `chia_createOffer`                    | `make_offer`                   |   no field    | `always`      |   ✅    |
| `chia_takeOffer`                      | `take_offer`                   |    `true`     | `always`      |   ✅    |
| `chia_cancelOffer`                    | `cancel_offer`                 |    `true`     | `auto_submit` |   ✅    |
| `chip0002_signCoinSpends`             | `sign_coin_spends`             |    `false`    | `always`      |   ✅    |
| `chip0002_signMessage`                | `sign_message_with_public_key` |   no field    | `always`      |   ✅    |
| `chia_signMessageByAddress`           | `sign_message_by_address`      |   no field    | `always`      |   ✅    |
| WC read-only (connect, chainId, etc.) | —                              |       —       | ❌            |    —    |

`chip0002_signCoinSpends` is the one case where `auto_submit: false` still prompts, because
`sign_coin_spends` signs before it looks at the flag.

## Matrix — app bridge operations

Bridge requests are gated in `process_after_approval`, not by the endpoint macro. The gate runs after
the user approves the request summary in the `bridge-approval` system app and before the handler
executes; on a protected wallet the approval runtime is hidden first, because app runtimes are
sibling webviews that would otherwise cover the main webview's dialog.

| Approval body           | Bridge method                | Gated | Target wallet        |
| ----------------------- | ---------------------------- | :---: | -------------------- |
| `SendXch`               | `wallet.sendXch`             |  ✅   | active               |
| `SignCoinSpends`        | `wallet.signCoinSpends`      |  ✅   | active               |
| `SignMessage`           | `wallet.signMessage`         |  ✅   | active               |
| `GetSecretKey`          | `wallet.getSecretKey`        |  ✅   | named by fingerprint |
| `CapabilityGrant`       | `app.requestCapabilityGrant` |  ❌   | —                    |
| `NetworkWhitelistGrant` | —                            |  ❌   | —                    |

`approval_requires_password` matches the body exhaustively with no catch-all, so a new approval body
must decide deliberately.

A password-protected wallet always gets an approval dialog even when the app holds the
`wallet.send_xch_auto_submit` capability. Silent auto-submit is incompatible with password
protection.

## Remaining design considerations

1. **Session caching asymmetry** — biometric caches for 5 minutes; password never caches. Session
   unlock is deferred; see the password-gate design's non-goals.
2. **`requires_password` is observable** — the `PasswordRequest` event can be received by an app
   webview, so it deliberately carries no wallet identity and no password. `requires_password` still
   tells a listener whether the active wallet is protected.
