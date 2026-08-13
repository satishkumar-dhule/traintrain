const fs = require('fs');

let code = fs.readFileSync('server.ts', 'utf-8');

const regex = /app\.get\("\/rail-api\/live-status", async \(req, res\) => \{[\s\S]*?\}\);/m;

const replacement = `app.get("/rail-api/live-status", async (req, res) => {
    const train = (req.query.train || '').trim();
    const dateStr = (req.query.date || '').trim();
    if (!train) return res.status(400).json({error: "Train number missing"});
    
    try {
      const result = await liveStatusService.getLiveStatus(train, dateStr);
      res.json({...result.data, data_source: result.source});
    } catch (err: any) {
      console.error("[API Error] Live status fetch failed:", err.message);
      res.status(500).json({ error: err.message || "Live status check failed." });
    }
  });`;

code = code.replace(regex, replacement);

fs.writeFileSync('server.ts', code);
