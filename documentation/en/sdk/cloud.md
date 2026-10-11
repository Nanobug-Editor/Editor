---
title: HTTP, secrets and private state
description: Bounded cloud queries, system credentials, private directories and disposable timers.
section: sdk
order: 19
alternate: /zh-cn/sdk/cloud/
---

# HTTP, secrets and private state

`network.http` 1.0.0, `storage.secrets` 1.0.0 and `lifecycle.timers` 1.0.0 are stable.
Directory operations, cache and temporary areas require stable `storage.private` 1.2.0.
The minimum host supports `protocol = 7`, `api.base = ^1` and these capability versions.
The common [compatibility and deprecation policy](/en/sdk/#stability-and-capability-compatibility)
applies. An unavailable required capability rejects the package; an unavailable optional
capability must disable the feature or use a plugin fallback. Browser OAuth and AI APIs are
not provided by these capabilities.

```json
{
  "protocol": 7,
  "api": {"base": "^1", "required": {
    "network.http": "^1", "storage.secrets": "^1",
    "lifecycle.timers": "^1", "storage.private": "^1.2"
  }},
  "permissions": ["network.http", "network.http:https://api.example.test", "secrets", "storage"]
}
```

## HTTP requests and streaming

`http::start(Request)` returns an instance-owned stream handle. The request accepts a method,
headers and an optional byte body; it is independent of image loading and native processes.
Use `http::read(&handle)` to pull one event and release queue capacity. `None` means pending.
Events are `Headers { status, headers }`, `Chunk { bytes }`, `Finished { bytes }` and
`Failed { error }`. Non-2xx status codes are ordinary response metadata. Headers precede body
chunks, and exactly one terminal event seals the stream. Further reads return `InvalidState`.
Close the handle with `api::guest::close_resource` after its terminal event.

```rust
let mut request = plugin_protocol::http::Request::get("https://api.example.test/query");
request.method = "POST".into();
request.body = "hello 世界😀".as_bytes().to_vec();
let stream = plugin_protocol::http::start(request)?;
// Retry a pending read with a disposable timer; consume one bounded event per call.
let event = plugin_protocol::http::read(&stream)?;
```

| Bound | Value |
| --- | --- |
| Request body | 256 KiB |
| Request headers | 64 entries, 16 KiB combined names and values |
| Response headers | 128 entries, 32 KiB complete wire header block |
| Response body | Requested limit from 1 byte to 16 MiB; default 1 MiB |
| Chunk and queue | At most 32 KiB per chunk, eight queued events |
| Deadline | 100–120,000 ms from acceptance, default 30,000 ms |
| Redirects | 0–5, default 3 |
| Producers | Four concurrent workers shared by the host's HTTP configuration |

Methods contain uppercase ASCII letters or hyphens, at most 32 bytes; `CONNECT` and `TRACE`
are denied. Header values accept printable ASCII and horizontal tab; encode Unicode content
in the body or URL. Host, framing, connection, upgrade and proxy authorization headers are
host-owned. URLs must be absolute HTTP(S), at most 4,096 bytes, without user-info, `@`, fragment
or control characters. Malformed requests fail before allocation. Size limits apply even
without a trustworthy Content-Length. A slow consumer pauses the bounded producer and still
consumes the original deadline.

Approve both `network.http` and the exact canonical origin permission, including a non-default
port when present. Wildcard destinations are unavailable. Every redirect checks the next
origin before connecting. HTTPS-to-HTTP downgrade is denied. Cross-origin redirects clear
all caller-supplied headers and reject forwarding a non-empty body. A 303, or a 301/302 for
POST, changes to GET with an empty body before this check. Relative redirects remain subject
to the same rules. Reading a secret grants no network permission.

The host selects its proxy and TLS roots; plugins cannot disable certificate or hostname
verification or inherit host proxy environment variables. The Windows host accepts the
explicit `NANOBUG_HTTP_PROXY` setting for a HTTP(S) CONNECT proxy. Without that setting it
connects directly. Standard WebPKI roots are used unless the embedding host supplies an
explicit root set. Unsupported proxy configuration, offline transport, DNS and TLS failures
produce a safe error category without echoing the URL or credential-bearing data.

`CancelRequest` on an HTTP handle stops waiting and shuts down its socket; the next read
returns terminal `Failed(Cancelled)`. The handle stays owned until `CloseResource` releases
it. Closing directly cancels without retaining a readable terminal result. Socket shutdown
also interrupts stalled TLS, CONNECT and body IO; socket IO checks revocation at most every
50 ms without extending the original deadline. OS DNS itself is synchronous: cancellation
and deadline expiry are observable to the consumer immediately, but an already-running DNS
call may occupy one of the four workers until the OS returns. It cannot open a late connection
after revocation. No additional unbounded DNS threads are created.

## System secrets

Negotiate `storage.secrets ^1` and approve the separate `secrets` permission. The SDK offers
`secrets::read(key)`, `write(key, bytes)` and `delete(key)`. Keys are non-empty, at most 128
UTF-8 bytes, without control characters; values are 1–2,560 bytes. A missing key reads as
`None`. Deletion is idempotent. Windows uses Generic Credentials persisted for the current
OS user, scoped by the host profile, plugin ID and key. Another plugin cannot name or read
this namespace, even when it also has `secrets`. No credential enumeration is exposed.

Other platforms return `UnsupportedOperation`; a failing Windows credential facility returns
`OperationFailed`. Neither case writes plaintext files. Secret writes and deletion require
committed activation; preparation, migration, failed candidates and delegated services cannot
modify them. Disable and trust revocation remove access while preserving stored credentials
for a later authorized activation. Explicit deletion removes them.

Use a password [native input](/en/sdk/interaction/) for Token entry. Treat a returned secret as
sensitive application data: never place it in ordinary settings, logs, views or snapshots.
HTTP and secret request Debug output omits sensitive payloads. Public service calls retain
caller network permissions, including target origins; they cannot access provider private
files or secrets, or borrow the provider's stronger grants. A provider must not forward its Token.

## Private directories, cache and temporary bytes

Approve `storage`. Existing `OpenData`, `ReadFile` and `WriteFile` remain available. With
`storage.private ^1.2`, use `storage::open(Area::Data | Cache | Temporary)` and
`storage::operate(Operation)`. The same owned handle supports file reads and writes.
`Stat`, `List`, `CreateDirectory`, `Rename` and `Delete` manage relative paths. List is a
sorted page of 1–128 entries; pass the last returned name as `after`, and inspect `has_more`.
Metadata reports the entry name, directory flag and file bytes; directory bytes are zero.
CreateDirectory requires an existing parent. Rename never overwrites. Delete requires an
explicit `recursive` flag for non-empty directories. Root deletion and renaming are denied.

All areas share the manifest byte quota, at most 1 MiB per file, 1,024 entries, 64 KiB of
combined entry-name bytes and depth 16. Paths are at most 4,096 bytes. Entry/name limits are
reserved before mutation; directories with no file bytes still count. Absolute paths,
traversal, Windows devices, alternate data streams, backslashes, symlinks and reparse points
are rejected. Initialization retains the existing transaction's direct-child staged writes
and maximum 64 staged files; directory management requires committed activation.

Data and cache persist in host-managed private storage outside the workspace and participate
in [transactional private-data replacement](/en/sdk/migration/). Temporary bytes are removed
when their instance retires. A failed native cleanup is reported while the retired instance's
access remains revoked. Handles from another plugin, scope or previous incarnation are invalid.

## Disposable timers and debounce

`timers::start(delay_ms, repeat)` returns an owned timer; `reset(&handle, delay_ms)` replaces
its next deadline. A delay is 10 ms–one day; at most 32 timers are live per instance. Each due
timer delivers `Notification::Timer { handle, sequence }`. Repeating timers coalesce missed
ticks and schedule their next tick from delivery; sequences increase per delivered tick.
At most eight oldest due timers run per poll, so every due timer receives a turn. Timer delivery
depends on host polling and is not a hard real-time guarantee. A one-shot handle is released
before its callback and cannot be reset after firing.

For debounce, retain one one-shot handle and reset it on each refresh intent before it fires.
Cancel or replace that intent by closing its pending debounce handle, so it cannot restart a query.
For periodic HTTP polling, retain a repeating handle and close it together with the stream
when complete or cancelled. Disable, trust revocation, update retirement and failed activation
revoke streams, timers and their service-origin authority. Recovery restores logical private
data with a new instance identity; it does not replay external HTTP effects or restore sockets.
