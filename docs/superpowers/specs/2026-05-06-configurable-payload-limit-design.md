# Configurable Payload Size Limit — Design Spec

**Date:** 2026-05-06
**Repo:** tauri-plugin-nostr-sync
**Status:** Approved

---

## Problem

The 64KB payload size limit is currently a hardcoded module-level constant in `src/state.rs`. There is no way for plugin consumers to increase it. Some use cases (e.g. larger wallet state blobs) may need more headroom up to the practical relay limit.

---

## Goal

Make the max payload size configurable via `PluginBuilder`, defaulting to 64KB and capped at 400KB. Misconfiguration (over-cap value) surfaces as a catchable error through Tauri's standard plugin `setup` closure mechanism — no panics, no silent clamping.

---

## Constants

Defined as `pub const` in `src/state.rs`:

```rust
pub const DEFAULT_PAYLOAD_LIMIT: usize = 64 * 1024;   // 64KB — default
pub const MAX_PAYLOAD_LIMIT: usize = 400 * 1024;       // 400KB — hard cap
```

Both are exposed publicly so host apps can reference them (e.g. for UI validation).

---

## Error Variant

Add to `src/error.rs`:

```rust
InvalidPayloadLimit { requested: usize, max: usize }
```

Display: `"payload limit {requested} exceeds maximum allowed {max}"`.

---

## Architecture

### `PluginBuilder` (`src/builder.rs`)

- New field: `max_payload_size: usize` (defaults to `DEFAULT_PAYLOAD_LIMIT`)
- New infallible setter: `.max_payload_size(bytes: usize) -> Self`
- `build()` signature unchanged — threads `max_payload_size` into the setup closure alongside the existing `relays`, `namespace`, `device_id`

### `desktop::init()` / `mobile::init()`

- Add `max_payload_size: usize` parameter
- Pass through to `NostrSyncState::new()`

### `NostrSyncState` (`src/state.rs`)

- New field: `max_payload_size: usize`
- `new(namespace, device_id, max_payload_size)` — validates `max_payload_size <= MAX_PAYLOAD_LIMIT`, returns `Err(Error::InvalidPayloadLimit { ... })` if not. Namespace validation runs first (existing order preserved).
- Remove module-level `PAYLOAD_LIMIT` const (replaced by the two public constants above)
- `check_payload_size` becomes a method (`&self`) using `self.max_payload_size` instead of the old const

### Error propagation path

```text
NostrSyncState::new()  →  Err(InvalidPayloadLimit)
  ↑ called by
desktop::init()  →  crate::Result<TauriPluginNostrSync>
  ↑ called by (with ?)
setup closure in PluginBuilder::build()  →  Result<(), Box<dyn Error>>
  ↑ propagated through
Tauri's Plugin::initialize()  →  catchable by host app at startup
```

No changes to public IPC commands or TypeScript bindings — this is purely a Rust-side builder and state machine concern.

`NostrSyncState::new()` is `pub` (re-exported from `lib.rs`), so adding the `max_payload_size` parameter is a breaking change to the public Rust API. Acceptable at `0.1.0-alpha`.

---

## Tests

### `src/state.rs`

- Update `payload_at_limit_is_accepted` and `payload_over_limit_is_rejected` to use instance state rather than the removed module const
- Add: `new_rejects_payload_limit_over_max` — verifies `InvalidPayloadLimit` error when `max_payload_size > MAX_PAYLOAD_LIMIT`
- Add: `new_accepts_payload_limit_at_max` — verifies `MAX_PAYLOAD_LIMIT` itself is accepted
- Add: `custom_limit_is_enforced` — creates state with a small custom limit, verifies payloads above it are rejected and below are accepted

### `src/builder.rs`

- Add: `max_payload_size_defaults_to_64kb`
- Add: `max_payload_size_setter_stores_value`

---

## Documentation

### `README.md`

Add `.max_payload_size(bytes)` to the builder example block and a short note explaining the default (64KB), the cap (400KB), and that exceeding the cap surfaces as an error at startup.

### `specs/tauri-plugin-nostr.md`

Update the "Plugin Registration" Rust snippet and add a row to the design constraints table:

| Constraint | Value |
| --- | --- |
| Default payload limit | 64KB |
| Maximum configurable limit | 400KB |
| Over-cap behavior | `Error::InvalidPayloadLimit` via setup closure |

---

## Out of Scope

- No changes to IPC commands or TypeScript bindings
- No per-category payload limits
- No runtime reconfiguration after `build()`
