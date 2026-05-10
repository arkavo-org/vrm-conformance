//! LSP-style stdio framing: `Content-Length: N\r\n\r\n<body>`. Same framing
//! MCP itself uses for stdio transports.

use std::io::{BufRead, BufReader, Read, Write};

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("missing or malformed Content-Length header")]
    MissingContentLength,
    #[error("invalid header line: {0}")]
    BadHeader(String),
}

pub fn write_message<W: Write>(w: &mut W, body: &[u8]) -> Result<(), FrameError> {
    write!(w, "Content-Length: {}\r\n\r\n", body.len())?;
    w.write_all(body)?;
    w.flush()?;
    Ok(())
}

pub fn read_message<R: Read>(r: &mut R) -> Result<Vec<u8>, FrameError> {
    let mut reader = BufReader::new(r);
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Err(FrameError::MissingContentLength);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        let (k, v) = trimmed
            .split_once(':')
            .ok_or_else(|| FrameError::BadHeader(trimmed.to_string()))?;
        if k.eq_ignore_ascii_case("Content-Length") {
            content_length = Some(
                v.trim()
                    .parse()
                    .map_err(|_| FrameError::BadHeader(trimmed.to_string()))?,
            );
        }
    }

    let len = content_length.ok_or(FrameError::MissingContentLength)?;
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    Ok(body)
}
