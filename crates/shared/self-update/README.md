# soma-self-update

`soma-self-update` is a standalone binary self-update transaction for Rust services. It has zero path-dependencies on the Soma workspace and can be copied into another repository wholesale.

## Scope

The crate owns artifact integrity checks, bounded staging, executable validation,
atomic Unix installation, durable confirmation state, and rollback.

## Non-goals

HTTP clients, authentication, release discovery, service orchestration, and
server-side artifact hosting remain adopter responsibilities.

## Safety boundary

Authenticate the directive independently or verify a detached signature before
starting an update. A same-channel SHA-256 digest does not authenticate a
hostile publisher.

## Platform support

Transport-neutral directive, staging, and validation APIs compile everywhere.
The included atomic installer and re-exec adapter support Unix only.

## API lifecycle

Construct an `UpdateDirective`, stage and validate its artifact, install it,
restart, recover pending state on startup, and confirm only after the new
service reports healthy.
