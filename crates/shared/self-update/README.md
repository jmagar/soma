# soma-self-update

`soma-self-update` is a standalone binary self-update transaction for Rust services. It has zero path-dependencies on the Soma workspace and can be copied into another repository wholesale.

## Scope

The crate owns artifact integrity checks, bounded staging, executable validation,
atomic Unix installation, durable confirmation state, and rollback.

The default policy streams at most 128 MiB and gives a staged executable 10
seconds to answer `--version`. Validation requires the advertised version as an
exact output token rather than a substring.

## Non-goals

Release discovery, HTTP and authentication, signature format and key
management, service orchestration, server-side artifact hosting, and automatic
Cortex or Soma integration remain adopter responsibilities.

## Safety boundary

Authenticate the directive independently or verify a detached signature before
starting an update. A same-channel SHA-256 digest does not authenticate a
hostile publisher. Verify any detached signature before calling `stage`.

Artifacts are same-origin by default and HTTPS-only. Loopback HTTP is available
only through explicit `HttpsOrLoopbackHttp` policy. URL references use URL
semantics, so endpoint `https://host/v1/heartbeats` plus
`/v1/agent/binary` resolves to `https://host/v1/agent/binary`, not a path under
the heartbeat endpoint.

## Platform support

Transport-neutral directive, staging, and validation APIs compile everywhere.
The included atomic installer and re-exec adapter support Unix only.
Non-Unix adopters can use directive, staging, and validation but the provided
installer reports `UnsupportedPlatform`; supply a platform-specific deployment
strategy there.

## API lifecycle

Construct an `UpdateDirective`, stage and validate its artifact, install it,
restart, recover pending state on startup, and confirm only after the new
service reports healthy.

Installation takes an advisory state lock, writes and syncs a durable marker,
retains a unique rollback backup, then atomically renames the verified artifact.
A process crash before confirmation leaves the marker and backup for startup
recovery. Each unconfirmed startup increments the marker; after the configured
threshold the backup is restored and the adopter must restart again. Successful
health confirmation removes both marker and backup. Corrupt markers, missing
backups, and cleanup failures are typed errors and retain diagnostic state where
possible. Operators should stop competing updater processes before repairing a
reported marker or backup path.

## Cortex extraction map

- Cortex `AgentUpdateDirective` maps to `UpdateDirective`.
- Cortex reqwest and bearer authentication stay in Cortex caller code.
- Cortex `maybe_update` splits into `stage` → `validate` → `install` → caller
  restart/re-exec.
- The first successful heartbeat calls `confirm_success`; startup calls
  `recover_on_startup` before entering the heartbeat loop.

See `examples/heartbeat_agent.rs` for a compile-checked lifecycle. The library
never reads global process arguments. Pass the arguments to preserve to
`restart_command` or `reexec`, or let a supervisor such as systemd restart the
service after `RestartRequired`.
