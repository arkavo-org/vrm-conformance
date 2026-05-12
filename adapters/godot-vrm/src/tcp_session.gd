# NDJSON request/response loop over a connected StreamPeerTCP socket.
# One JSON object per line, terminated by "\n". On socket close or
# read error, returns cleanly so main.gd can call quit(0).

class_name TcpSession

const Operations := preload("res://src/operations.gd")
const Session := preload("res://src/session.gd")

# Run the loop on `socket`. Blocks until the peer (shim) closes the
# connection. The Session is held in main.gd; we forward both to dispatch.
static func run(tree: SceneTree, session: Session, socket: StreamPeerTCP) -> void:
    var buf := PackedByteArray()
    while true:
        if socket.get_status() != StreamPeerTCP.STATUS_CONNECTED:
            return
        socket.poll()
        var available := socket.get_available_bytes()
        if available > 0:
            var chunk := socket.get_data(available)
            if chunk[0] != OK:
                push_error("tcp read error: %d" % chunk[0]); return
            buf.append_array(chunk[1])
        var newline_byte := 0x0a
        while true:
            var nl := buf.find(newline_byte)
            if nl < 0: break
            var line: PackedByteArray = buf.slice(0, nl)
            buf = buf.slice(nl + 1)
            var text := line.get_string_from_utf8()
            var parsed: Variant = JSON.parse_string(text)
            var resp: Dictionary
            if parsed == null or typeof(parsed) != TYPE_DICTIONARY:
                resp = {
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": "parse error" },
                }
            else:
                var req: Dictionary = parsed
                var raw_id: Variant = req.get("id", null)
                var id_out: Variant = raw_id
                if typeof(raw_id) == TYPE_FLOAT and raw_id == floor(raw_id):
                    id_out = int(raw_id)
                resp = await Operations.dispatch(
                    tree, session,
                    id_out, req.get("method", ""), req.get("params", {}),
                )
            var out := (JSON.stringify(resp) + "\n").to_utf8_buffer()
            var put_err := socket.put_data(out)
            if put_err != OK:
                push_error("tcp write error: %d" % put_err); return
        OS.delay_msec(5)
