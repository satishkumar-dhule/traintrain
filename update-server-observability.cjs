const fs = require('fs');

let serverCode = fs.readFileSync('server.ts', 'utf-8');

const newEndpoint = `
  app.get("/rail-api/observability", (req, res) => {
    // Generate simulated metrics for the Google Underground Engine
    res.json({
      active_connections: Math.floor(Math.random() * 5) + 15, // around 20
      latency_ms: Math.floor(Math.random() * 40) + 10,
      req_per_sec: Math.floor(Math.random() * 100) + 200,
      cpu_usage: Math.floor(Math.random() * 15) + 5,
      mem_usage: Math.floor(Math.random() * 20) + 80
    });
  });

  // Vite middleware
`;

serverCode = serverCode.replace('// Vite middleware', newEndpoint);
fs.writeFileSync('server.ts', serverCode);
