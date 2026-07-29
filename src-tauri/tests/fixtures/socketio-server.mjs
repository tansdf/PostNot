import { createServer } from "node:http";
import { Server } from "socket.io";

const path = "/custom-socket/";
const httpServer = createServer();
const io = new Server(httpServer, {
  path,
  transports: ["polling", "websocket"],
  allowUpgrades: true
});

let connectionNumber = 0;

io.of("/admin").on("connection", (socket) => {
  connectionNumber += 1;
  socket.emit("connection-meta", {
    auth: socket.handshake.auth,
    query: socket.handshake.query.fixture,
    header: socket.handshake.headers["x-postnot-fixture"],
    transport: socket.conn.transport.name,
    connectionNumber
  });

  socket.on("echo", (...args) => {
    const maybeAck = args.at(-1);
    if (typeof maybeAck === "function") {
      args.pop();
      maybeAck(...args);
    }
  });

  socket.on("binary-echo", (payload, ack) => {
    if (typeof ack === "function") ack(payload);
  });

  socket.on("drop-transport", () => {
    socket.conn.transport.close();
  });

  socket.on("drop-server", () => {
    httpServer.close();
    socket.conn.transport.close();
  });
});

httpServer.listen(0, "127.0.0.1", () => {
  const address = httpServer.address();
  process.stdout.write(`${JSON.stringify({ port: address.port, path })}\n`);
});

function shutdown() {
  io.close(() => httpServer.close(() => process.exit(0)));
}

process.on("SIGTERM", shutdown);
process.on("SIGINT", shutdown);
