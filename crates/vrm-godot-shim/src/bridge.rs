//! stdio <-> TCP-loopback forwarding loop.
//!
//! Reads framed JSON-RPC bodies from stdin via `vrm_ops::stdio`, writes
//! each body + "\n" to the TCP socket, reads one "\n"-terminated response
//! from the socket, and writes it back to stdout via `vrm_ops::stdio`.
//! On stdin EOF, returns Ok(()).

use std::io::{BufRead, BufReader, Read, Write};

use thiserror::Error;
use vrm_ops::stdio::{self, FrameError};

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("framing: {0}")]
    Framing(#[from] FrameError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("godot side closed the socket mid-request")]
    PeerClosed,
}

/// Forward one round of: stdin frame -> TCP line -> TCP line -> stdout frame.
/// Returns `Ok(true)` on a successful round, `Ok(false)` on clean stdin EOF.
pub fn forward_one<R, W, S>(
    stdin: &mut R,
    stdout: &mut W,
    tcp: &mut S,
) -> Result<bool, BridgeError>
where
    R: Read,
    W: Write,
    S: Read + Write,
{
    let body = match stdio::read_message(stdin) {
        Ok(b) => b,
        // `vrm_ops::stdio::read_message` signals stream EOF (zero-byte read
        // before any header line) as `MissingContentLength`. Treat that as a
        // clean shutdown; other framing errors — including a truncated body
        // surfacing as `Io(UnexpectedEof)` from `read_exact` — surface to
        // the caller as `BridgeError::Framing` so silent corruption is loud.
        Err(FrameError::MissingContentLength) => return Ok(false),
        Err(e) => return Err(BridgeError::Framing(e)),
    };
    tcp.write_all(&body)?;
    tcp.write_all(b"\n")?;
    tcp.flush()?;

    let mut reader = BufReader::new(tcp);
    let mut response = String::new();
    let n = reader.read_line(&mut response)?;
    if n == 0 {
        return Err(BridgeError::PeerClosed);
    }
    // Strip exactly one trailing '\n' (BufRead::read_line keeps it).
    if response.ends_with('\n') {
        response.pop();
    }
    stdio::write_message(stdout, response.as_bytes())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    /// Stand up a one-shot TCP server that reads one line and writes a
    /// canned response. Returns the port the server is listening on.
    fn one_shot_server(canned_response: &'static str) -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = String::new();
            let mut reader = BufReader::new(&stream);
            reader.read_line(&mut buf).unwrap();
            drop(reader);
            stream.write_all(canned_response.as_bytes()).unwrap();
            stream.write_all(b"\n").unwrap();
        });
        port
    }

    #[test]
    fn round_trip_request_response_through_tcp() {
        let port = one_shot_server(r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#);
        let mut tcp = TcpStream::connect(("127.0.0.1", port)).unwrap();

        // Frame a request and feed it into the bridge as stdin.
        let request = br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}"#;
        let mut framed = Vec::new();
        stdio::write_message(&mut framed, request).unwrap();
        let mut stdin = Cursor::new(framed);
        let mut stdout: Vec<u8> = Vec::new();

        let ok = forward_one(&mut stdin, &mut stdout, &mut tcp).unwrap();
        assert!(ok);

        // Decode the framed output and confirm the canned response made it back.
        let body = stdio::read_message(&mut Cursor::new(&stdout)).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed["result"]["ok"], true);
    }

    #[test]
    fn eof_on_stdin_returns_false() {
        let port = one_shot_server("{}");
        let mut tcp = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let mut stdin: &[u8] = &[];
        let mut stdout: Vec<u8> = Vec::new();
        let result = forward_one(&mut stdin, &mut stdout, &mut tcp).unwrap();
        assert!(!result, "expected false on EOF");
        drop(tcp);
    }
}
