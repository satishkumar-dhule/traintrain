const fs = require('fs');

let serverCode = fs.readFileSync('server.ts', 'utf-8');

// Add train search logic
const newRoutes = `
  app.get("/rail-api/search/trains", (req, res) => {
    const q = (req.query.q || "").toLowerCase().trim();
    try {
      const raw = require('fs').readFileSync('data/trains.json', 'utf-8');
      const trains = JSON.parse(raw);
      if (!q) {
        return res.json(trains.slice(0, 10));
      }
      const matches = trains.filter(t => t.number.startsWith(q) || t.name.toLowerCase().includes(q)).slice(0, 15);
      res.json(matches);
    } catch(err) {
      res.json([]);
    }
  });

  app.get("/rail-api/search/stations", (req, res) => {
    const q = (req.query.q || "").toLowerCase().trim();
    if (!q) {
      return res.json(stationsService.search("ndls").slice(0, 10));
    }
    const matches = stationsService.search(q);
    res.json(matches.slice(0, 15));
  });

  // Vite middleware
`;

serverCode = serverCode.replace('// Vite middleware', newRoutes);
fs.writeFileSync('server.ts', serverCode);
