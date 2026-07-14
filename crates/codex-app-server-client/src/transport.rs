//! Wire transport: the app-server's stdio mode is newline-delimited JSON
//! (JSONL) with the bare JSON-RPC 2.0 message shape and the `"jsonrpc":"2.0"`
//! header omitted (see <https://developers.openai.com/codex/app-server>).
//! This module only frames/deframes lines; message typing lives in
//! [`crate::protocol`] and dispatch lives in [`crate::client`].

use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::protocol::RequestId;
use crate::{Error, Result};

/// Hard cap on a single NDJSON line's size. Without this, a single huge or
/// unterminated line from a buggy or malicious app-server peer would grow
/// [`read_line`]'s buffer without bound. 64 MiB comfortably covers legitimate
/// large payloads (e.g. `fs/readFile` results, big diffs) while still being a
/// real, finite bound.
pub const MAX_LINE_BYTES: usize = 64 * 1024 * 1024;

/// A reply to a server->client request, queued for the writer task.
pub(crate) enum OutgoingReply {
    Result {
        id: RequestId,
        result: serde_json::Value,
    },
    Error {
        id: RequestId,
        code: i64,
        message: String,
        data: Option<serde_json::Value>,
    },
}

impl OutgoingReply {
    pub(crate) fn into_line(self) -> Result<String> {
        let value = match self {
            OutgoingReply::Result { id, result } => {
                serde_json::json!({ "id": id, "result": result })
            }
            OutgoingReply::Error {
                id,
                code,
                message,
                data,
            } => {
                serde_json::json!({ "id": id, "error": { "code": code, "message": message, "data": data } })
            }
        };
        Ok(serde_json::to_string(&value)?)
    }
}

/// Spawns `command app-server [extra_args...]` with stdio piped, ready for
/// [`crate::CodexAppServerClient::connect`].
pub(crate) fn spawn_app_server(
    command: &str,
    extra_args: &[String],
) -> Result<(ChildStdin, BufReader<ChildStdout>, Child)> {
    let mut cmd = Command::new(command);
    cmd.arg("app-server");
    cmd.args(extra_args);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::inherit());
    cmd.kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|source| Error::Spawn {
        command: command.to_string(),
        source,
    })?;

    let stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    Ok((stdin, BufReader::new(stdout), child))
}

pub(crate) async fn write_line<W>(writer: &mut W, line: &str) -> std::io::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    writer.write_all(line.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await
}

/// Reads one `\n`-terminated line into `buf` (cleared first, but its
/// allocation is reused rather than replaced - callers keep one persistent
/// `buf` across the whole read loop so this stays allocation-free after the
/// first few calls), enforcing [`MAX_LINE_BYTES`]. Returns the number of
/// bytes read (0 on clean EOF with nothing left to read), or an error if the
/// line is invalid UTF-8 or would exceed the cap - in either case the caller
/// should treat the connection as dead rather than try to resynchronize
/// mid-line.
pub(crate) async fn read_line<R>(reader: &mut R, buf: &mut String) -> std::io::Result<usize>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    let mut bytes = std::mem::take(buf).into_bytes();
    bytes.clear(); // drops content, keeps the allocated capacity
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            break; // EOF
        }
        if let Some(pos) = available.iter().position(|&b| b == b'\n') {
            bytes.extend_from_slice(&available[..=pos]);
            reader.consume(pos + 1);
            break;
        }
        let n = available.len();
        if bytes.len() + n > MAX_LINE_BYTES {
            reader.consume(n);
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("NDJSON line exceeded the {MAX_LINE_BYTES}-byte cap"),
            ));
        }
        bytes.extend_from_slice(available);
        reader.consume(n);
    }
    if bytes.is_empty() {
        return Ok(0);
    }
    *buf = String::from_utf8(bytes)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    Ok(buf.len())
}

// Re-export the trait bounds so callers of `CodexAppServerClient::connect` don't
// need to import tokio themselves for common cases.
pub use tokio::io::{AsyncBufRead, AsyncWrite};

#[cfg(unix)]
pub(crate) fn split_unix_stream(
    stream: tokio::net::UnixStream,
) -> (
    tokio::net::unix::OwnedWriteHalf,
    BufReader<tokio::net::unix::OwnedReadHalf>,
) {
    let (read_half, write_half) = stream.into_split();
    (write_half, BufReader::new(read_half))
}
