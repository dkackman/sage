# Password Gate — Manual Smoke Test Checklist

**Branch:** `password-gate`
**Status:** outstanding — no automated coverage exists for these paths

Every automated gate on this branch passes: workspace build (warning-free), full test
suite, the `sage-rpc` canary, clippy `-D warnings`, `cargo fmt`, eslint, prettier, and
the frontend build. What follows cannot be automated: the repository has no frontend
test runner, and these paths need a human at a running app.

Launch with:

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
pnpm run tauri:dev
```

## 1. Main window, protected wallet

- [ ] Send a small amount of XCH. **No dialog appears while the transaction is being
      built** — the password dialog comes up only after Submit on the confirmation
      dialog, and exactly once. Two dialogs for one send means an endpoint is filed
      as `always` in `password-gating.json` when it should be `auto_submit`.
- [ ] Enter the wrong password twice. It re-prompts each time, showing the remaining
      attempt count, and reads "1 attempt remaining" (singular) on the last try.
- [ ] Enter the correct password. The transaction proceeds.
- [ ] Repeat with three wrong passwords. A toast reads "Too many incorrect password
      attempts" and the dialog closes. **It must not fail silently.**
- [ ] Start a send and dismiss the dialog. No error toast appears and nothing is submitted.

## 2. Main window, unprotected wallet

- [ ] Send XCH. No dialog appears at all, at either stage.
- [ ] Mobile only, biometrics enabled: the biometric gate appears, and a second
      operation within five minutes does not re-prompt.

## 3. Logged-out wallet list — the regression the final review caught

These two broke completely and silently at one point; they are the highest-value checks here.

- [ ] Log out. On the wallet list, open "Wallet details" on an **unprotected** wallet.
      It must work.
- [ ] Same, on a **protected** wallet — including one that is not the last-active
      wallet. It must prompt for **that wallet's** password, not another's.
- [ ] Delete a protected wallet from the logged-out list. Same expectation.
- [ ] While logged in to wallet A, delete or inspect protected wallet B. The prompt
      must name B and accept B's password.

## 4. App bridge, protected wallet

- [ ] Launch an app that calls `wallet.sendXch`. The `bridge-approval` app shows the
      transaction summary.
- [ ] Approving hides that app and reveals the password dialog **in the main window** —
      not inside the app's webview.
- [ ] The correct password completes the request; the app receives success.
- [ ] Cancelling the dialog fails the request; the app receives an error.
- [ ] Grant the app `WalletSendXchAutoSubmit` and repeat: an approval **still** appears,
      because the wallet is protected.
- [ ] Queue two approvals and answer the first. The second must remain reachable —
      the approval window must not be left hidden.

## 5. App bridge, unprotected wallet

- [ ] With the auto-submit grant, send XCH: no approval appears.
- [ ] Approve a `wallet.getSecretKey` request: the approval window must **not** flicker
      away and back for a prompt that never comes.

## 6. WalletConnect

- [ ] Pair a dApp against a protected wallet. A signing request prompts for the password
      in the main window and completes.

## 7. Timeout

- [ ] Start a gated operation and leave the prompt untouched for five minutes. A toast
      reads "Password prompt timed out". A late Submit or Cancel must not throw an
      unhandled rejection in the console.

## 8. Trust boundary (optional, verifies a known limitation)

The `password-request` event targets the `main` webview, but Tauri's listener matching
short-circuits for `EventTarget::Any` listeners, so an app webview holding
`core:event:allow-listen` can observe it. The payload was reduced to carry nothing that
identifies a wallet, and the password itself only ever returns via
`submit_password_response`, which is ACL-denied to app webviews.

- [ ] In an installed app's devtools, listen for `password-request` and trigger a gated
      operation. Confirm the payload contains **no** `fingerprint` — only `requestId`,
      `requiresPassword`, `attempt`, and `error`.
