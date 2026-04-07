# Passkey (WebAuthn PRF) — Pre-Merge Checklist

This document covers everything required before merging the `passkey` passkey branch into `main`.
All temporary dev credentials must be swapped for sage's real identifiers, and the RP endpoint
must be live and correctly configured.

---

## 1. Apple Developer Portal

### 1a. Register the App ID

At https://developer.apple.com/account/resources/identifiers/list:

- **Bundle ID:** `com.rigidnetwork.sage`
- Enable the **Associated Domains** capability

If the App ID already exists, just confirm Associated Domains is enabled.

### 1b. Create a Production Provisioning Profile

At https://developer.apple.com/account/resources/profiles/list:

- **Type:** macOS App Development (dev) or Mac App Distribution (release)
- **App ID:** `com.rigidnetwork.sage`
- Include all developer certificates and Mac devices needed for testing
- Download and replace `embedded.provisionprofile` in the sage root

---

## 2. RP Endpoint — `sage.rigidnetwork.com`

WebAuthn requires an Apple App Site Association (AASA) file served from the RP domain.

### 2a. Host the AASA file

Serve at: `https://sage.rigidnetwork.com/.well-known/apple-app-site-association`

Content:
```json
{
  "webcredentials": {
    "apps": ["NQJQRYZZG3.com.rigidnetwork.sage"]
  }
}
```

Requirements:
- HTTPS only (valid cert, no redirects)
- `Content-Type: application/json`
- No authentication required — Apple's CDN fetches this during app install
- The path must be exactly `/.well-known/apple-app-site-association`

### 2b. Verify the AASA

After deploying, verify at: https://branch.io/resources/aasa-validator/ or:
```bash
curl -s https://sage.rigidnetwork.com/.well-known/apple-app-site-association | python3 -m json.tool
```

---

## 3. Code Changes

Four files have temporary dev credentials that must be updated.

### 3a. `src/hooks/usePasskey.ts` — lines 4–5

```typescript
// Before (dev)
const RP_ID = 'webauthn.dkackman.com';

// After
const RP_ID = 'sage.rigidnetwork.com';
```

The `ORIGIN` constant derives from `RP_ID` automatically — no separate change needed.

### 3b. `src-tauri/Entitlements.plist`

```xml
<!-- Before (dev) -->
<key>com.apple.application-identifier</key>
<string>86TDY6D9V2.net.kackman.webauthn.example</string>
<key>com.apple.developer.team-identifier</key>
<string>86TDY6D9V2</string>
<key>com.apple.developer.associated-domains</key>
<array>
    <string>webcredentials:webauthn.dkackman.com?mode=developer</string>
</array>

<!-- After -->
<key>com.apple.application-identifier</key>
<string>NQJQRYZZG3.com.rigidnetwork.sage</string>
<key>com.apple.developer.team-identifier</key>
<string>NQJQRYZZG3</string>
<key>com.apple.developer.associated-domains</key>
<array>
    <string>webcredentials:sage.rigidnetwork.com</string>
</array>
```

Note: Remove `?mode=developer` — that bypass is only for development. Production relies on
Apple fetching the AASA from the live endpoint.

Also remove `com.apple.security.get-task-allow` if this is a release/distribution build
(it allows debugger attachment and should not be in production).

### 3c. `src-tauri/tauri.conf.json`

```json
// Before (dev)
"identifier": "net.kackman.webauthn.example"

// After
"identifier": "com.rigidnetwork.sage"
```

### 3d. `build-macos-dev.sh`

```bash
# Before (dev)
BUNDLE_ID="net.kackman.webauthn.example"

# After
BUNDLE_ID="com.rigidnetwork.sage"
```

---

## 4. Clean Up Debug Logging

Remove temporary console logging from `src/hooks/usePasskey.ts`:

```typescript
// Remove this line in setupPasskey:
console.log('[usePasskey] register response:', JSON.stringify(regResult));

// Remove this line in the catch block:
console.error('[usePasskey] setupPasskey error:', e);
```

The `authenticatePasskey` catch can also drop its `console.error` or reduce to a no-op,
since returning `undefined` on failure is the documented behavior.

---

## 5. Tauri Capability — Release Signing

The `src-tauri/capabilities/desktop.json` already has `"webauthn:default"` — no change needed.

For the release build the app must be signed and notarized with the production profile.
The `build-macos-dev.sh` script is development-only. Tauri's normal release pipeline
(`pnpm tauri build`) will need the provisioning profile and entitlements wired in via
`tauri.conf.json`. Add to the macOS section of `tauri.conf.json`:

```json
"macOS": {
  "entitlements": "Entitlements.plist",
  "provisioningProfile": "embedded.provisionprofile"
}
```

Confirm the signing identity in Tauri's config matches the certificate in the provisioning profile.

---

## 6. Verification Checklist

Before merging:

- [ ] AASA file live at `https://sage.rigidnetwork.com/.well-known/apple-app-site-association`
- [ ] AASA contains `NQJQRYZZG3.com.rigidnetwork.sage`
- [ ] App ID `com.rigidnetwork.sage` registered with Associated Domains in Apple Developer portal
- [ ] Provisioning profile updated for `com.rigidnetwork.sage`
- [ ] All four code files updated with production identifiers
- [ ] `?mode=developer` removed from associated-domains entitlement
- [ ] `com.apple.security.get-task-allow` removed from release entitlements
- [ ] Debug `console.log` removed from `usePasskey.ts`
- [ ] Full passkey flow tested with production build: setup → wallet re-open (PRF decrypt) → remove

---

## Summary of Credential Swap

| What | Dev (current) | Production |
|------|--------------|------------|
| RP ID / domain | `webauthn.dkackman.com` | `sage.rigidnetwork.com` |
| Bundle ID | `net.kackman.webauthn.example` | `com.rigidnetwork.sage` |
| Team ID | `86TDY6D9V2` | `NQJQRYZZG3` |
| Associated domain | `webcredentials:webauthn.dkackman.com?mode=developer` | `webcredentials:sage.rigidnetwork.com` |
| App identifier | `86TDY6D9V2.net.kackman.webauthn.example` | `NQJQRYZZG3.com.rigidnetwork.sage` |
