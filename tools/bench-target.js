// Target HTTP rápido y sin dependencias para el benchmark (Node).
// Responde 200 con un JSON pequeño y fijo a cualquier método/ruta.
// Uso: node tools/bench-target.js [puerto]   (por defecto 8899)
const http = require("http");
const body = Buffer.from(JSON.stringify({ ok: true, id: 123, token: "abc123" }));

const server = http.createServer((req, res) => {
  if (req.method === "GET" || req.method === "HEAD") {
    send(res);
  } else {
    req.on("data", () => {});
    req.on("end", () => send(res));
  }
});

function send(res) {
  res.writeHead(200, { "Content-Type": "application/json", "Content-Length": body.length });
  res.end(body);
}

const port = parseInt(process.argv[2] || "8899", 10);
server.keepAliveTimeout = 60000;
server.maxConnections = 100000;
server.listen(port, "127.0.0.1", () => console.log("bench-target on 127.0.0.1:" + port));
