# godot-vrm adapter — Godot-side entry. Reads the loopback port from the
# first positional user arg (after `--`), connects to vrm-godot-shim,
# and runs the NDJSON session loop until the shim closes the socket.

extends SceneTree

const TcpSession := preload("res://src/tcp_session.gd")

func _init() -> void:
    var args := OS.get_cmdline_user_args()
    if args.is_empty():
        push_error("godot-vrm adapter: expected positional port arg after `--`"); quit(2); return
    var port := args[0].to_int()
    if port <= 0 or port > 65535:
        push_error("godot-vrm adapter: bad port: %s" % args[0]); quit(2); return

    var socket := StreamPeerTCP.new()
    var err := socket.connect_to_host("127.0.0.1", port)
    if err != OK:
        push_error("godot-vrm adapter: connect_to_host failed: %d" % err); quit(2); return

    var deadline := Time.get_ticks_msec() + 5000
    while socket.get_status() == StreamPeerTCP.STATUS_CONNECTING:
        if Time.get_ticks_msec() > deadline:
            push_error("godot-vrm adapter: connect timeout"); quit(2); return
        socket.poll()
        OS.delay_msec(10)
    if socket.get_status() != StreamPeerTCP.STATUS_CONNECTED:
        push_error("godot-vrm adapter: not connected: status=%d" % socket.get_status()); quit(2); return

    TcpSession.run(socket)
    socket.disconnect_from_host()
    quit(0)
