# PostNot patch

This directory vendors the published `rust_socketio` 0.6.0 crate with two
targeted corrections:

`Packet::new_from_payload` previously encoded an outgoing binary event as a
Socket.IO `BinaryAck` packet whenever an acknowledgement id was present.
Socket.IO protocol v5 requires it to remain a `BinaryEvent`; the id asks the
server to acknowledge that event. The old packet was ignored by a Socket.IO
4.8.1 server, so binary emits with ACK always timed out.

The interoperability fixture in
`src-tauri/tests/fixtures/socketio-server.mjs` regression-tests the corrected
binary event and acknowledgement round trip.

The asynchronous client now also exposes `on_reconnect_failed`, invoked once
when its configured reconnect attempts are exhausted, and
`on_connection_closed`, invoked when its packet stream ends without a
reconnect. Upstream only logged exhaustion and kept the polling task alive,
and it exposed no terminal callback for non-reconnecting clients. Those
behaviors left consumers unable to release dead sessions or leave a
`Reconnecting`/`Connected` UI state. PostNot uses the callbacks to transition
the session to a terminal status; its local fixture tests successful
reconnection, terminal exhaustion, and non-reconnecting transport loss.
