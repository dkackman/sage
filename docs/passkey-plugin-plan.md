# tauri-plugin-webauthn Fork: iOS & PRF Extension

## Context

This document describes a plan to fork [Profiidev/tauri-plugin-webauthn](https://github.com/Profiidev/tauri-plugin-webauthn) and add iOS support and the WebAuthn PRF extension across all platforms.

**macOS support is largely done** — [PR #44](https://github.com/Profiidev/tauri-plugin-webauthn/pull/44) by @fendent adds a complete macOS authenticator using `ASAuthorizationPlatformPublicKeyCredentialProvider` with a Swift ↔ Rust C FFI bridge via `swift-rs`. The maintainer has reviewed it positively and is looking to merge. The fork should be based on this PR's branch or incorporate its changes.

The end goal is to use this plugin in the [Sage wallet](https://github.com/dkackman/sage) (Tauri-based Chia blockchain wallet) to provide passkey-based encryption of wallet key material, as a mutually exclusive alternative to password-based encryption.

The fork author has prior experience building Tauri plugins with native platform bridges — see [tauri-plugin-secure-element](https://github.com/dkackman/tauri-plugin-secure-element) for reference (Swift/Kotlin/CNG bridges, hardware-backed crypto).

## Why PRF Matters (Not Just Authentication)

### Standard WebAuthn (what the plugin does today)

Standard WebAuthn is challenge-response identity verification:

```
Registration:
  Authenticator generates key pair (private stays on device)
  Returns: public key + credential ID → stored by relying party

Authentication:
  Relying party sends random challenge
  Authenticator signs challenge with private key
  Returns: signature → relying party verifies
```

The output is a **yes/no identity proof**. No secret material is produced. This is sufficient for web login but **useless for encryption** — you can't derive an AES key from a signature verification result.

Without PRF, passkeys can only act as a **UI gate** (like mobile biometric auth). The actual key material on disk remains encrypted with an empty password and is trivially recoverable by anyone with file access.

### WebAuthn + PRF Extension

PRF (Pseudo-Random Function) is defined in the [WebAuthn Level 3 spec](https://w3c.github.io/webauthn/#prf-extension). It piggybacks on the authentication ceremony to produce **deterministic secret output** derived from the authenticator's private key:

```
Authentication with PRF:
  Relying party sends: challenge + PRF salt (arbitrary bytes)
  Authenticator: signs challenge (normal) AND computes HMAC(private_key, salt)
  Returns: signature + PRF output (32 bytes of secret material)
```

PRF output properties:
- **Deterministic**: Same credential + same salt → same 32 bytes every time
- **Secret**: Derived from the hardware-bound private key
- **Unique per credential**: Different passkey → different output with same salt
- **Unique per salt**: Different salt → different output from same passkey
- **Full entropy**: 32 bytes of cryptographically strong material — no key stretching needed

This 32-byte output can directly serve as an encryption key (or be fed into HKDF to derive one), making passkeys a genuine replacement for password-based encryption.

### Security Comparison

| Property | Password | Passkey (gate only) | Passkey + PRF |
|----------|----------|-------------------|---------------|
| Key material encrypted at rest | Yes | No | Yes |
| Resistant to file theft | Yes | No | Yes |
| Phishing resistant | No | Yes | Yes |
| No secret to remember | No | Yes | Yes |
| Hardware-bound | No | Yes | Yes |
| Brute-force resistant | Depends on password | N/A | Yes |
| Recoverable if device lost | Yes (re-enter password) | N/A | No* |

*For a crypto wallet, recovery is via mnemonic seed phrase backup — not the encryption method.

## Current Plugin Architecture (Including PR #44)

```
src/
├── authenticators/
│   ├── ctap2/           # Linux (USB/NFC FIDO2 keys)
│   │   ├── event.rs     # PIN prompt events (Linux-specific)
│   │   ├── platform.rs  # CTAP2 protocol implementation
│   │   └── mod.rs
│   ├── macos.rs         # macOS (PR #44 — ASAuthorization via Swift FFI)
│   ├── mobile.rs        # Android (Google FIDO2 API)
│   ├── windows.rs       # Windows Hello / Windows WebAuthn API
│   └── mod.rs           # Platform dispatch
├── commands.rs          # Tauri command definitions
├── error.rs             # Error types
└── lib.rs               # Plugin init

macos/                   # Swift package (PR #44)
├── Package.swift
└── Sources/WebauthnBridge/
    ├── Exports.swift        # C-callable FFI functions, JSON serialization
    └── PasskeyHandler.swift # ASAuthorizationController wrapper
```

**Platform support (with PR #44):**
| Platform | Supported | Authenticator |
|----------|:---------:|---------------|
| Linux    | Yes       | ctap2/        |
| Windows  | Yes       | windows.rs    |
| Android  | Yes       | mobile.rs     |
| macOS    | Yes (PR #44) | macos.rs   |
| iOS      | No        | —             |

**Key dependencies:**
- `webauthn-rs-proto` — WebAuthn type definitions
- `webauthn-authenticator-rs` — CTAP2 authenticator implementation (used for Linux)
- `swift-rs` — Rust ↔ Swift bridge (macOS, build dependency)
- `base64urlsafedata`, `base64` — base64url encoding/decoding
- `tokio`, `serde`, `openssl`

**PRF support today:** None. No PRF-related types, parameters, or extension handling exist in the plugin. In both `parse_registration_response` and `parse_authentication_response` in `macos.rs`, the `extensions` field is set to `Default::default()`.

## What PR #44 Provides (macOS Authenticator)

### Architecture

The macOS implementation uses a C FFI callback pattern:

```
Tauri command (Rust)
  → macos.rs: calls webauthn_register/webauthn_authenticate via extern "C"
  → Exports.swift: receives C args, dispatches to MainActor
  → PasskeyHandler.swift: runs ASAuthorizationController ceremony
  → Exports.swift: serializes ASAuthorization response to JSON, invokes callback
  → macos.rs: parses JSON into webauthn-rs-proto types via mpsc::channel
```

### Key implementation details

- **FFI bridge**: `@_cdecl` exported Swift functions (`webauthn_register`, `webauthn_authenticate`, `webauthn_free_string`) called from Rust `extern "C"` declarations
- **Async pattern**: Rust creates `mpsc::channel`, passes sender as `u64` context to Swift. Swift callback reconstitutes the sender via `Box::from_raw` and sends the result. Rust side uses `recv_timeout` with the WebAuthn timeout parameter.
- **Dual authenticator support**: Both `ASAuthorizationPlatformPublicKeyCredentialProvider` (passkeys/iCloud Keychain) and `ASAuthorizationSecurityKeyPublicKeyCredentialProvider` (USB/NFC keys) are offered in a single `ASAuthorizationController`
- **Security key algorithm**: Hard-coded to ES256 via `ASAuthorizationPublicKeyCredentialParameters(algorithm: .ES256)`
- **Presentation anchor**: `NSApplication.shared.windows.first ?? NSWindow()` — works for single-window Tauri apps
- **Build integration**: `swift-rs::SwiftLinker` in `build.rs` with `.with_package("WebauthnBridge", "macos")`, targeting macOS 13.0

### Requirements for consuming apps (from PR #44's README)

The macOS passkey support requires significant Apple developer setup:
- Developer ID Application certificate + code signing with hardened runtime
- `Entitlements.plist` with `com.apple.application-identifier`, `com.apple.developer.team-identifier`, `com.apple.developer.associated-domains`
- Provisioning profile with Associated Domains capability
- Apple App Site Association (AASA) file hosted at `https://yourdomain.com/.well-known/apple-app-site-association`
- App must be notarized — macOS won't process associated domain entitlements for un-notarized Developer ID apps
- For dev builds outside `/Applications`: must register with LaunchServices via `lsregister -f`

### Known limitations (from PR #44)

- Only iCloud Keychain appears in the ASAuthorization sheet — third-party credential providers (1Password, etc.) don't yet implement macOS Credential Provider API
- `?mode=developer` associated domains bypass doesn't work with Developer ID-signed apps (only Xcode development-signed builds)
- AASA must be live and cached by Apple's CDN for Developer ID apps

## Work Plan

### Phase 1: iOS Authenticator (~1 week)

The macOS implementation from PR #44 provides most of the pattern. iOS uses the same AuthenticationServices framework.

**What to adapt from macOS:**

| macOS (PR #44) | iOS equivalent |
|----------------|----------------|
| `NSApplication.shared.windows.first` | `UIApplication` shared key window |
| `ASPresentationAnchor` = `NSWindow` | `ASPresentationAnchor` = `UIWindow` |
| `import AppKit` | `import UIKit` |
| `macos/` Swift package | Tauri's existing `ios/` plugin directory convention |

**Files to create/modify:**
- `ios/Sources/WebauthnPlugin/` — iOS Swift bridge, adapting from `macos/Sources/WebauthnBridge/`
- `src/authenticators/ios.rs` — Rust side, very similar to `macos.rs`
- Or: update `src/authenticators/mobile.rs` to handle iOS alongside Android (the current `#[cfg(mobile)]` gating)
- Update `src/authenticators/mod.rs` — add iOS dispatch
- Update `build.rs` — Swift compilation for iOS target

**Key differences from macOS:**
- iOS uses Tauri's built-in iOS plugin architecture (`ios_path("ios")` already in `build.rs`) rather than a standalone Swift package
- Associated domains are configured in the Xcode project capabilities, not just entitlements
- The existing `mobile.rs` handles Android — decide whether iOS goes there (with `#[cfg(target_os = "ios")]` blocks) or gets its own file. A separate `ios.rs` is cleaner since iOS uses Swift FFI like macOS while Android uses JNI.

**Approach:** Copy the `PasskeyHandler.swift` and `Exports.swift` pattern from macOS, adjust for `UIKit` and iOS presentation context. The Rust side (`ios.rs`) can be nearly identical to `macos.rs` — same FFI signatures, same JSON parsing, same `mpsc::channel` pattern.

### Phase 2: PRF Extension — All Platforms (~2-3 weeks)

This is the cross-cutting change. PRF must be added to the plugin's type system and to every platform authenticator.

#### 2a. Plugin API Types

Add PRF input/output types to the plugin's public API:

```rust
/// PRF salt values provided during registration or authentication
pub struct PrfValues {
    pub first: Vec<u8>,              // 32-byte salt
    pub second: Option<Vec<u8>>,     // optional second salt
}

/// PRF input for registration ceremony
pub struct PrfRegistrationInput {
    pub eval: Option<PrfValues>,
}

/// PRF output from registration (indicates support)
pub struct PrfRegistrationOutput {
    pub enabled: bool,
}

/// PRF input for authentication ceremony
pub struct PrfAssertionInput {
    pub eval: PrfValues,
    pub eval_by_credential: Option<HashMap<Vec<u8>, PrfValues>>,
}

/// PRF output from authentication — the usable secret material
pub struct PrfAssertionOutput {
    pub first: Vec<u8>,              // 32-byte derived key
    pub second: Option<Vec<u8>>,     // if second salt was provided
}
```

These types should be added to the registration and authentication request/response structs that the plugin already defines.

#### 2b. Apple — macOS and iOS (macOS 15+ / iOS 18+)

Apple added PRF support in 2024:
- `ASAuthorizationPublicKeyCredentialPRFRegistrationInput` — attach to registration request
- `ASAuthorizationPublicKeyCredentialPRFAssertionInput` — attach to assertion request
- Responses include PRF output bytes

**Where to hook in (referencing PR #44 code):**

1. **Registration** — In `PasskeyHandler.swift`'s `register()` method, after creating `platformRequest`:
   ```swift
   if #available(macOS 15.0, iOS 18.0, *) {
       // Attach PRF input to platformRequest (NOT securityKeyRequest — PRF is platform-only)
       let prfInput = ASAuthorizationPublicKeyCredentialPRFRegistrationInput()
       // or with eval values if provided
       platformRequest.prf = prfInput
   }
   ```

2. **Authentication** — In `PasskeyHandler.swift`'s `authenticate()` method, after creating `platformRequest`:
   ```swift
   if #available(macOS 15.0, iOS 18.0, *) {
       let prfInput = ASAuthorizationPublicKeyCredentialPRFAssertionInput(
           inputValues: ASAuthorizationPublicKeyCredentialPRFAssertionInputValues(
               saltInput1: prfSaltData  // 32 bytes from Rust
           )
       )
       platformRequest.prf = prfInput
   }
   ```

3. **Response extraction** — In `Exports.swift`'s `registrationJSON()` and `assertionJSON()`:
   ```swift
   if #available(macOS 15.0, iOS 18.0, *),
      let prfResult = assertion.prf {
       json["prf"] = [
           "first": prfResult.first.base64URLEncodedString(),
           // "second" if present
       ]
   }
   ```

4. **FFI signatures** — `webauthn_register` and `webauthn_authenticate` in `Exports.swift` need additional parameters for PRF salt bytes. Add optional `prf_salt_ptr`/`prf_salt_len` parameters (null/0 when PRF not requested).

5. **Rust JSON parsing** — In `macos.rs`'s `parse_registration_response()` and `parse_authentication_response()`, extract PRF output from the JSON and populate the `extensions` field instead of `Default::default()`.

**Important:** PRF only works with **platform authenticators** (passkeys), not security keys. The `PasskeyHandler` creates both platform and security key requests in a single `ASAuthorizationController` — PRF inputs must only be attached to the platform request.

**Availability:** PRF requires macOS 15+ / iOS 18+. The Swift bridge must:
1. Check `#available(macOS 15.0, iOS 18.0, *)`
2. Return a clear "PRF not supported" indicator when unavailable (e.g., `prf: null` in JSON response)
3. Include PRF inputs/outputs only when the OS supports them

#### 2c. Windows

Windows Hello supports the `hmac-secret` CTAP2.1 extension, which is what PRF wraps at the protocol level. The existing `windows.rs` needs to:
- Pass `hmac-secret` extension data through the Windows WebAuthn API
- Map the extension output to plugin PRF types

#### 2d. Linux (CTAP2)

The `webauthn-authenticator-rs` crate (already a dependency) may already support `hmac-secret` extension at the CTAP2 level. Investigation needed:
- Check if `hmac-secret` is supported in the crate's CTAP2 implementation
- If yes, wire it through `ctap2/platform.rs`
- If no, implement `hmac-secret` extension handling in the CTAP2 command layer

#### 2e. Android

Google's Credential Manager API has added PRF support. The existing `mobile.rs` (which uses Android FIDO2 APIs) needs:
- PRF extension parameters in the authentication request
- PRF output extraction from the response
- Version/capability detection since PRF support varies by Android version

### Phase 3: API Surface & JavaScript Bindings

Update the plugin's JavaScript/TypeScript API to expose PRF:

```typescript
interface RegisterOptions {
  // ...existing fields...
  extensions?: {
    prf?: {
      eval?: { first: ArrayBuffer; second?: ArrayBuffer };
    };
  };
}

interface AuthenticateOptions {
  // ...existing fields...
  extensions?: {
    prf?: {
      eval: { first: ArrayBuffer; second?: ArrayBuffer };
      evalByCredential?: Record<string, { first: ArrayBuffer; second?: ArrayBuffer }>;
    };
  };
}

interface AuthenticationResult {
  // ...existing fields...
  extensions?: {
    prf?: {
      results?: { first: ArrayBuffer; second?: ArrayBuffer };
    };
  };
}
```

This mirrors the [Web Authentication API extensions](https://w3c.github.io/webauthn/#dictdef-authenticationextensionsprfvalues) format, keeping the plugin's goal of being "a nearly drop-in replacement for @simplewebauthn/browser."

## How Sage Will Use This Plugin

Once the plugin supports macOS/iOS and PRF, Sage will integrate it as one of three mutually exclusive protection paths:

```
Protection check order: passkey → password → biometric → none

Path 1 - Biometric (mobile, gate-only):
  Existing @tauri-apps/plugin-biometric
  No encryption involvement, UI gate only
  keys.bin encrypted with empty password

Path 2 - Password:
  Existing Argon2 + AES-256-GCM flow
  password → Argon2(password, salt) → AES key → decrypt key material

Path 3 - Passkey + PRF:
  New flow using this plugin
  passkey ceremony → PRF(private_key, stored_salt) → HKDF → AES key → decrypt key material
```

### Sage Integration Points (for reference, not part of plugin work)

| Sage Layer | File | Change |
|-----------|------|--------|
| Encryption | `crates/sage-keychain/src/encrypt.rs` | Add `encryption_key_from_prf()` — HKDF instead of Argon2 |
| Key data | `crates/sage-keychain/src/key_data.rs` | Add `passkey_credential_id`, `passkey_prf_salt` to `Secret` |
| API | `crates/sage/src/endpoints/keys.rs` | Passkey registration/auth endpoints |
| Frontend | `src/contexts/PasswordContext.tsx` | New passkey branch in `requestPassword()` |
| Settings | `src/pages/Settings.tsx` | Passkey enrollment UI |

### Sage Encryption Flow with PRF

```rust
// In encrypt.rs — new key derivation path alongside existing Argon2 path

// Existing: password-based
fn encryption_key_from_password(password: &[u8], salt: &[u8]) -> Key<Aes256Gcm> {
    // Argon2(password, salt) → 32 bytes
}

// New: PRF-based
fn encryption_key_from_prf(prf_output: &[u8; 32]) -> Key<Aes256Gcm> {
    // HKDF-SHA256(prf_output, info="sage-wallet-encryption") → 32 bytes
    // No Argon2 needed — PRF output already has full entropy
}
```

Key data storage additions:
```rust
// In key_data.rs
Secret {
    master_pk: [u8; 48],
    entropy: bool,
    encrypted: Encrypted,           // existing: {ciphertext, nonce, salt}
    password_protected: bool,       // existing
    // New fields for passkey
    passkey_credential_id: Option<Vec<u8>>,  // WebAuthn credential ID
    passkey_prf_salt: Option<[u8; 32]>,      // salt sent to PRF during auth
}
```

### Recovery Considerations

Passkey + PRF encryption means **losing the authenticator loses the encryption key**. For Sage this is acceptable because:
- Users are expected to back up their mnemonic seed phrase at wallet creation
- Recovery path: re-import wallet from mnemonic, set up new passkey
- The enrollment UI must clearly communicate this to users

## Platform Support Matrix (Target State)

| Platform | Auth | PRF | Min OS Version |
|----------|:----:|:---:|----------------|
| macOS | Yes (PR #44) | Yes | macOS 13 (auth), macOS 15 (PRF) |
| iOS | Yes | Yes | iOS 16 (auth), iOS 18 (PRF) |
| Windows | Yes | Yes | Windows 10 1903+ |
| Linux | Yes | TBD | Depends on CTAP2.1 key support |
| Android | Yes | TBD | API 28+ (auth), PRF varies |

On platforms/versions where PRF is unavailable, the plugin should clearly indicate this so the consuming app can fall back to an alternative protection method (password or biometric gate).

## Estimated Effort

| Phase | Effort | Notes |
|-------|--------|-------|
| ~~macOS authenticator~~ | ~~2-3 weeks~~ **Done** | PR #44 covers this |
| iOS authenticator | ~1 week | Adapt macOS Swift bridge for UIKit |
| PRF extension (all platforms) | 2-3 weeks | Cross-cutting; Apple is most straightforward |
| JS/TS API updates | ~2 days | Type additions, mirrors WebAuthn spec |
| Sage integration | ~2 weeks | Encryption path, UI, enrollment flow |
| **Total** | **~5-6 weeks** | Down from 7-9 thanks to PR #44 |

## Testing Strategy

- **macOS/iOS:** Test on physical devices with Touch ID / Face ID (Simulator has limited passkey support)
- **PRF:** Verify deterministic output — same credential + same salt must produce identical 32-byte output across multiple calls
- **Cross-platform:** Ensure credential IDs and PRF salts are portable in the data format (even though credentials themselves are device-bound)
- **Fallback:** Verify graceful behavior when PRF is unsupported (older OS, authenticator without hmac-secret)
- **Example app:** Extend the plugin's existing `examples/webauthn` to demonstrate PRF registration and authentication
- **Sage-specific:** Verify full round-trip: enroll passkey → encrypt key material with PRF → close app → reopen → authenticate with passkey → decrypt key material → sign transaction
