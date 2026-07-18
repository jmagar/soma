//! Instance (container/VM) CRUD, lifecycle, and snapshots. Implemented by
//! the "instances resource" task. Deliberately excludes exec, console
//! attach, and file push/pull - those use a different WebSocket-secrets
//! protocol than this crate's generic operations/events model and are
//! out of scope for this epic.
