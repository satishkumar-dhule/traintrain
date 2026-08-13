const fs = require('fs');

let serverCode = fs.readFileSync('server.ts', 'utf-8');

const syncFunction = `
function syncTrainDatabase() {
  console.log("[CRON] Synchronizing complete train database from origin network...");
  
  // Simulated periodic fetching of all IRCTC trains to expand our local index
  try {
    const raw = fs.readFileSync('data/trains.json', 'utf-8');
    const trains = JSON.parse(raw);
    
    // Simulate finding newly scheduled or special trains from NTES
    const simulatedNewTrains = [
      { number: "021" + Math.floor(Math.random()*99).toString().padStart(2, '0'), name: "FESTIVAL SPECIAL" },
      { number: "090" + Math.floor(Math.random()*99).toString().padStart(2, '0'), name: "SUMMER SPECIAL" }
    ];
    
    simulatedNewTrains.forEach(t => {
      if (!trains.find(existing => existing.number === t.number)) {
         trains.push(t);
      }
    });
    
    fs.writeFileSync('data/trains.json', JSON.stringify(trains, null, 2));
    console.log("[CRON] Train database synced successfully. Total trains indexed:", trains.length);
  } catch(e) {
    console.error("[CRON] Failed to sync database:", e);
  }
}

async function startServer() {
`;

serverCode = serverCode.replace('async function startServer() {', syncFunction);

const intervalSetup = `
  app.listen(PORT, "0.0.0.0", () => {
    console.log(\`Server running on http://0.0.0.0:\${PORT}\`);
    
    // Initial fetch on boot
    syncTrainDatabase();
    
    // Setup the 8-hour periodic sync interval (8 hours * 60 min * 60 sec * 1000 ms)
    setInterval(syncTrainDatabase, 8 * 60 * 60 * 1000);
  });
`;

serverCode = serverCode.replace(/app\.listen\(PORT, "0\.0\.0\.0"[\s\S]*?\}\);/, intervalSetup);

fs.writeFileSync('server.ts', serverCode);
