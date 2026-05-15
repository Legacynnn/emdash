# 29. SSH foundation (openssh, control sockets)

Date: 2026-05-15

## Status

Accepted (EMD-10 v1). Defers PTY-over-SSH and several adjacent
features to follow-ups.

## Context

The Electron build's `core/ssh/*` is ~1700 LOC of TypeScript using
the `ssh2` JS library, which:

- Implements the SSH protocol in JavaScript (no system `ssh`
  dependency).
- Manages its own authentication: password (us-stored),
  key-from-disk (us-read), agent (us-relayed).
- Maintains a connection pool with health checks and auto-reconnect.
- Exposes SFTP for directory listing (`controller.listFiles`).
- Parses `~/.ssh/config` itself (`sshConfigParser.ts`).
- Renders ssh debug logs into the connection-test UI.

Porting that surface 1:1 into Rust would mean pulling in `russh`
or similar (in-process SSH implementations), reimplementing the
auth and config-parsing layers, plus an SFTP client. Big surface,
a lot of subtle behavior, and we'd be one of very few production
users of the in-process Rust SSH stack.

Helmor took the alternate route: delegate to the system `ssh`
binary via `openssh` crate's control-socket multiplexing. We
follow the same path.

## Decision

1. **`openssh = "0.10"` with `native-mux` (no in-process SSH
   stack).** Every connect spawns one long-lived `ssh -N` control
   master per saved connection id; subsequent ops (`exec`, future
   PTY, future SFTP) reuse the socket. Trade-off: requires a
   POSIX `ssh` on PATH. Windows returns
   `SshError::Unsupported("…")` at runtime — not a panic, just a
   blocked operation.

2. **Don't store passwords as credentials we actively use.**
   AEAD-encrypted password storage exists (`SshCredentials`,
   keyed on connection id), but `openssh` doesn't have a path to
   inject a password into the system `ssh` short of writing an
   `askpass` helper. We keep the password column for round-trip
   compatibility with the Electron schema, but key + agent are
   the primary auth paths. Passphrase-protected keys without
   ssh-agent aren't supported in v1.

3. **Authoritative `~/.ssh/config` is the user's, parsed by
   `ssh`.** We don't ship our own parser. If a user has a
   `Host alias` block, our `host` field is the alias and the
   real address resolution happens in the system `ssh`. This is
   a behavior-equivalent simplification of the Electron source's
   `sshConfigParser`.

4. **Connection manager is a `parking_lot::Mutex<HashMap>` keyed
   on connection id.** Idempotent `connect`. Sessions live as
   `Arc<SshClient>` so concurrent users (e.g. PTY spawn + remote
   exec) share the control socket without re-handshaking.
   Disconnect drops the entry; the `Arc` count goes to zero and
   `openssh::Session` closes the socket.

5. **Domain types prefixed `Ssh*` to avoid specta collisions.**
   `SshConnection`, `SshError`, `NewSshConnection`,
   `ConnectionState`, `ConnectionTestResult`. The
   `ConnectionState` and `ConnectionTestResult` types are
   intentionally not `Ssh`-prefixed because they're already
   scoped under the SSH module and a prefix would be redundant
   from the renderer's import path.

6. **DB schema is unchanged.** The `ssh_connections` table was
   pre-shipped in the collapsed bootstrap migration (EMD-7's
   schema port), so this PR is a pure module + commands add.

## v1 scope cuts

The following are intentionally deferred so this PR stays
reviewable and the foundation lands cleanly:

- **PTY over SSH.** Needs `portable-pty` + ssh argv plumbing,
  and the agent-spawn site (EMD-27 branch) is the consumer.
  Coordination cost is higher than implementation cost. Tracked
  for an EMD-10 v2 PR.
- **SFTP file listing.** `openssh` exposes `Sftp` but the
  directory walking + entry sorting is its own type-shape +
  serialization story. Small follow-up; not blocking on anything.
- **Health monitoring + auto-reconnect.** The Electron source's
  `getHealthStates` powers a renderer-side connection-status
  indicator that doesn't yet exist on the Tauri side. Defer
  until there's a UI consumer.
- **`testConnection` debug logs.** `openssh` doesn't surface the
  wire-level ssh debug stream. If users need it, we can shell out
  to `ssh -vv -G <host>` separately and parse the output, but
  that's a UI-driven feature not v1 scope.

## Consequences

- ~750 LOC of Rust + 9 Tauri commands. Substantially smaller
  than the Electron source's 1700 LOC because we delegate auth +
  config parsing to the system `ssh`.
- Three new `UiMutationEvent` variants:
  `SshConnectionSaved`, `SshConnectionDeleted`,
  `SshConnectionStateChanged`. The renderer's cache invalidation
  becomes one switch arm per variant.
- The `openssh` crate is a direct dep; the system `ssh` binary
  is a runtime requirement (POSIX only). Windows users see a
  graceful "not supported" envelope at the IPC boundary; the rest
  of the app remains functional.
- 15 new unit tests, all passing locally without network access
  (the unreachable-host test uses RFC 5737 TEST-NET-1).
- Wire-format snapshot regenerates (new commands + new event
  variants); no breaking change to existing commands.
