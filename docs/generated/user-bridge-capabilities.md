# User bridge capabilities

## `persistent_storage`

**Persistent storage**

Allows the app to store data on this device between sessions.

| Flag | Value |
|---|---|
| Requestable by app | `true` |
| User grantable | `true` |
| Shared with app | `true` |
| Externally observable | `false` |
| Accesses sensitive secret | `false` |

## `bridge.send`

**Bridge messaging**

Allows the app to send messages through the Sage bridge. (Only for sandbox tests)

| Flag | Value |
|---|---|
| Requestable by app | `true` |
| User grantable | `false` |
| Shared with app | `true` |
| Externally observable | `false` |
| Accesses sensitive secret | `false` |

## `app.get_capabilities`

**Read granted capabilities**

Allows the app to read the capabilities currently visible to it.

| Flag | Value |
|---|---|
| Requestable by app | `true` |
| User grantable | `false` |
| Shared with app | `true` |
| Externally observable | `false` |
| Accesses sensitive secret | `false` |

## `app.get_info`

**Read app information**

Allows the app to read its Sage app identity and permission information.

| Flag | Value |
|---|---|
| Requestable by app | `true` |
| User grantable | `false` |
| Shared with app | `true` |
| Externally observable | `false` |
| Accesses sensitive secret | `false` |

## `app.lifecycle.ready_to_stop`

**Acknowledge app shutdown**

Allows the app to acknowledge that it is ready to stop after a lifecycle request.

| Flag | Value |
|---|---|
| Requestable by app | `true` |
| User grantable | `false` |
| Shared with app | `true` |
| Externally observable | `false` |
| Accesses sensitive secret | `false` |

## `app.lifecycle.set_before_stop_listener`

**Listen before app shutdown**

Allows the app to register a before-stop lifecycle listener.

| Flag | Value |
|---|---|
| Requestable by app | `true` |
| User grantable | `false` |
| Shared with app | `true` |
| Externally observable | `false` |
| Accesses sensitive secret | `false` |

## `app.request_capability_grant`

**Request additional capability**

Allows the app to request a capability grant after installation.

| Flag | Value |
|---|---|
| Requestable by app | `true` |
| User grantable | `false` |
| Shared with app | `true` |
| Externally observable | `false` |
| Accesses sensitive secret | `false` |

## `app.request_network_whitelist_grant`

**Request network access**

Allows the app to request access to an additional network target after installation.

| Flag | Value |
|---|---|
| Requestable by app | `true` |
| User grantable | `false` |
| Shared with app | `true` |
| Externally observable | `false` |
| Accesses sensitive secret | `false` |

## `wallet.send_xch`

**Send XCH**

Allows the app to request XCH transactions from your wallet.

| Flag | Value |
|---|---|
| Requestable by app | `true` |
| User grantable | `true` |
| Shared with app | `true` |
| Externally observable | `true` |
| Accesses sensitive secret | `false` |

## `wallet.send_xch_auto_submit`

**Automatic XCH send**

Allows the app to submit XCH transactions without asking for per-transaction approval.

| Flag | Value |
|---|---|
| Requestable by app | `false` |
| User grantable | `false` |
| Shared with app | `false` |
| Externally observable | `false` |
| Accesses sensitive secret | `false` |

