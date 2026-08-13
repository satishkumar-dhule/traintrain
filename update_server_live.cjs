const fs = require('fs');
let content = fs.readFileSync('server.ts', 'utf-8');

if (!content.includes('LiveStatusService')) {
  content = content.replace(
    'import { ScheduleService } from "./src/modules/schedule/schedule.service";',
    'import { ScheduleService } from "./src/modules/schedule/schedule.service";\nimport { LiveStatusService } from "./src/modules/live-status/live-status.service";'
  );
  
  content = content.replace(
    'const scheduleService = new ScheduleService();',
    'const scheduleService = new ScheduleService();\n  const liveStatusService = new LiveStatusService();'
  );
  
  const liveRoute = `
  app.get("/rail-api/live-status", async (req, res) => {
    const train = (req.query.train as string)?.trim();
    try {
      const data = await liveStatusService.getLiveStatus(train);
      res.json(data);
    } catch (err: any) {
      console.error("[API Error] Live Status fetch failed:", err.message);
      res.status(500).json({ error: err.message || "Live status fetch failed." });
    }
  });
`;
  
  content = content.replace(
    'app.get("/rail-api/stations", (req, res) => {',
    liveRoute + '\n  app.get("/rail-api/stations", (req, res) => {'
  );
  fs.writeFileSync('server.ts', content);
}
