use vrm_ops::stdio::{read_message, write_message};

#[test]
fn round_trips_a_message() {
    let payload = br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}"#;

    let mut buf = Vec::new();
    write_message(&mut buf, payload).unwrap();

    let mut cursor = std::io::Cursor::new(buf);
    let read = read_message(&mut cursor).unwrap();
    assert_eq!(read, payload);
}

#[test]
fn rejects_missing_content_length() {
    let raw = b"\r\n\r\n{}";
    let mut cursor = std::io::Cursor::new(&raw[..]);
    let err = read_message(&mut cursor).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("content-length"));
}
