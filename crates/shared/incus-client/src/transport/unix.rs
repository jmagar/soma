//! Minimal hand-rolled HTTP/1.1 client over a Unix domain socket. Opens one
//! fresh `UnixStream` per request rather than pooling - see the module-level
//! design notes in the implementation plan this was built from
//! (`docs/superpowers/plans/2026-07-17-incus-client-crate.md`): a local
//! socket connect is cheap, and a fresh connection per request makes
//! concurrent requests trivially independent of each other, unlike a shared,
//! mutex-guarded stream would be.

use std::os::unix::fs::FileTypeExt;
use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::error::{Error, Result};
use crate::transport::Method;

/// Hard cap on a response body, enforced regardless of framing
/// (`Content-Length` or chunked). Mirrors `codex-app-server-client`'s
/// `MAX_LINE_BYTES` precedent.
pub const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

/// A parsed HTTP response: status code, headers (lowercased names), and the
/// raw body bytes. Envelope (Incus sync/async/error JSON) parsing happens
/// one layer up, in `crate::transport`.
#[derive(Debug, Clone)]
pub(crate) struct RawResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl RawResponse {
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Confirms `path` is actually a Unix domain socket before we ever try to
/// connect to it, so a stale regular file or wrong path fails with a clear
/// error instead of an opaque connection-refused.
pub(crate) fn check_is_socket(path: &Path) -> Result<()> {
    let metadata = std::fs::metadata(path).map_err(Error::Transport)?;
    if !metadata.file_type().is_socket() {
        return Err(Error::Transport(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{} is not a Unix domain socket", path.display()),
        )));
    }
    Ok(())
}

/// Executes one HTTP request over a fresh connection to `socket_path`,
/// capping the response body at [`MAX_RESPONSE_BYTES`].
pub(crate) async fn execute(
    socket_path: &Path,
    method: Method,
    path: &str,
    query: &[(&str, &str)],
    body: Option<&[u8]>,
    if_match: Option<&str>,
) -> Result<RawResponse> {
    execute_capped(
        socket_path,
        method,
        path,
        query,
        body,
        if_match,
        MAX_RESPONSE_BYTES,
    )
    .await
}

/// [`execute`]'s implementation, parameterized over the cap so tests can
/// exercise the boundary condition without a 64 MiB fixture.
pub(crate) async fn execute_capped(
    socket_path: &Path,
    method: Method,
    path: &str,
    query: &[(&str, &str)],
    body: Option<&[u8]>,
    if_match: Option<&str>,
    max_response_bytes: usize,
) -> Result<RawResponse> {
    check_is_socket(socket_path)?;
    let stream = UnixStream::connect(socket_path)
        .await
        .map_err(Error::Transport)?;
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    let request_line = build_request_line(method, path, query);
    write_half
        .write_all(request_line.as_bytes())
        .await
        .map_err(Error::Transport)?;

    write_half
        .write_all(b"Host: localhost\r\nAccept: application/json\r\nConnection: close\r\n")
        .await
        .map_err(Error::Transport)?;

    if let Some(etag) = if_match {
        write_half
            .write_all(format!("If-Match: {etag}\r\n").as_bytes())
            .await
            .map_err(Error::Transport)?;
    }

    if let Some(body) = body {
        write_half
            .write_all(
                format!(
                    "Content-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .map_err(Error::Transport)?;
        write_half.write_all(body).await.map_err(Error::Transport)?;
    } else {
        write_half
            .write_all(b"\r\n")
            .await
            .map_err(Error::Transport)?;
    }
    write_half.flush().await.map_err(Error::Transport)?;

    read_response(&mut reader, max_response_bytes).await
}

fn build_request_line(method: Method, path: &str, query: &[(&str, &str)]) -> String {
    if query.is_empty() {
        format!("{} {} HTTP/1.1\r\n", method.as_str(), path)
    } else {
        let query_string = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(query)
            .finish();
        format!("{} {}?{} HTTP/1.1\r\n", method.as_str(), path, query_string)
    }
}

async fn read_response<R>(reader: &mut BufReader<R>, max_bytes: usize) -> Result<RawResponse>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let status_line = read_line(reader, max_bytes).await?;
    let status = parse_status_line(&status_line)?;

    let mut headers = Vec::new();
    loop {
        let line = read_line(reader, max_bytes).await?;
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }

    let content_length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok());
    let is_chunked = headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("transfer-encoding") && value.eq_ignore_ascii_case("chunked")
    });

    let body = if let Some(length) = content_length {
        if length > max_bytes {
            return Err(Error::ResponseTooLarge { limit: max_bytes });
        }
        let mut buf = vec![0u8; length];
        reader
            .read_exact(&mut buf)
            .await
            .map_err(Error::Transport)?;
        buf
    } else if is_chunked {
        read_chunked_body(reader, max_bytes).await?
    } else {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let n = reader.read(&mut chunk).await.map_err(Error::Transport)?;
            if n == 0 {
                break;
            }
            if buf.len() + n > max_bytes {
                return Err(Error::ResponseTooLarge { limit: max_bytes });
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        buf
    };

    Ok(RawResponse {
        status,
        headers,
        body,
    })
}

async fn read_chunked_body<R>(reader: &mut BufReader<R>, max_bytes: usize) -> Result<Vec<u8>>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut body = Vec::new();
    loop {
        let size_line = read_line(reader, max_bytes).await?;
        let size = usize::from_str_radix(size_line.trim(), 16).map_err(|_| {
            Error::InvalidResponse(format!("invalid chunk size line: {size_line:?}"))
        })?;
        if size == 0 {
            // Consume the trailing CRLF after the terminating 0-size chunk.
            let _ = read_line(reader, max_bytes).await?;
            break;
        }
        if body.len() + size > max_bytes {
            return Err(Error::ResponseTooLarge { limit: max_bytes });
        }
        let mut chunk = vec![0u8; size];
        reader
            .read_exact(&mut chunk)
            .await
            .map_err(Error::Transport)?;
        body.extend_from_slice(&chunk);
        // Each chunk is followed by a CRLF that isn't part of the payload.
        let _ = read_line(reader, max_bytes).await?;
    }
    Ok(body)
}

/// Reads one `\r\n`-terminated line (the `\r\n` stripped from the returned
/// string), enforcing `max_bytes` so a peer that never sends a newline can't
/// grow the buffer without bound.
async fn read_line<R>(reader: &mut BufReader<R>, max_bytes: usize) -> Result<String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = reader.read(&mut byte).await.map_err(Error::Transport)?;
        if n == 0 {
            if buf.is_empty() {
                return Err(Error::Transport(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "connection closed before a complete response was received",
                )));
            }
            break;
        }
        if buf.len() + 1 > max_bytes {
            return Err(Error::ResponseTooLarge { limit: max_bytes });
        }
        if byte[0] == b'\n' {
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
            break;
        }
        buf.push(byte[0]);
    }
    String::from_utf8(buf)
        .map_err(|err| Error::InvalidResponse(format!("response line was not valid UTF-8: {err}")))
}

fn parse_status_line(line: &str) -> Result<u16> {
    let mut parts = line.split_whitespace();
    let _http_version = parts
        .next()
        .ok_or_else(|| Error::InvalidResponse(format!("empty status line: {line:?}")))?;
    let status = parts
        .next()
        .ok_or_else(|| Error::InvalidResponse(format!("malformed status line: {line:?}")))?;
    status
        .parse::<u16>()
        .map_err(|_| Error::InvalidResponse(format!("non-numeric status code: {status:?}")))
}

#[cfg(test)]
#[path = "unix_tests.rs"]
pub(crate) mod tests;
