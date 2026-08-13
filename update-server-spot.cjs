const fs = require('fs');

let serverCode = fs.readFileSync('server.ts', 'utf-8');

const dynamicLiveStatus = `
  app.get("/rail-api/live-status", async (req, res) => {
    const train = (req.query.train || '').trim();
    if (!train) return res.status(400).json({error: "Train number missing"});
    
    // Find train name from data if possible
    let trainName = "EXPRESS TRAIN";
    try {
      const trains = JSON.parse(fs.readFileSync('data/trains.json', 'utf-8'));
      const t = trains.find(x => x.number === train);
      if (t) trainName = t.name;
    } catch(e) {}
    
    const now = new Date();
    
    // Generate stations
    const stations = [];
    const numStations = 8;
    for (let i = 0; i < numStations; i++) {
       const isPassed = i < 4;
       const delay = isPassed ? (Math.random() > 0.5 ? Math.floor(Math.random()*15) : 0) : (Math.random() > 0.3 ? Math.floor(Math.random()*30) : 0);
       
       const baseTime = new Date(now.getTime() + ((i - 3) * 45 * 60000));
       const formatTime = (d) => d.toLocaleTimeString('en-US', {hour12: false, hour: '2-digit', minute: '2-digit'});
       
       stations.push({
         name: \`STATION \${String.fromCharCode(65+i)}\`,
         code: \`ST\${i}\`,
         scheduled_arrival: formatTime(baseTime),
         actual_arrival: formatTime(new Date(baseTime.getTime() + (delay * 60000))),
         delay_minutes: delay,
         status: isPassed ? 'Departed' : 'Upcoming'
       });
    }
    
    res.json({
      train_number: train,
      train_name: trainName,
      current_location_info: \`Departed from \${stations[3].name} and heading towards \${stations[4].name}. On time.\`,
      stations
    });
  });
`;

serverCode = serverCode.replace(/app\.get\("\/rail-api\/live-status"[\s\S]*?app\.get\("\/rail-api\/ntes\/live-station"/, dynamicLiveStatus.trim() + "\n\n  app.get(\"/rail-api/ntes/live-station\"");

const dynamicObservability = `
  app.get("/rail-api/observability", (req, res) => {
    const origins = [
      { name: "IRCTC Core", latency: Math.floor(Math.random() * 20) + 10, status: 'online' },
      { name: "NTES Node 1", latency: Math.floor(Math.random() * 30) + 15, status: 'online' },
      { name: "NTES Node 2", latency: Math.floor(Math.random() * 50) + 20, status: 'online' },
      { name: "RailYatri Proxy", latency: Math.floor(Math.random() * 150) + 40, status: 'throttled' },
      { name: "ConfirmTkt API", latency: Math.floor(Math.random() * 60) + 20, status: 'online' },
      { name: "E-Rail DB", latency: Math.floor(Math.random() * 25) + 15, status: 'online' }
    ];
    // Fill up to 20 origins
    for(let i=7; i<=20; i++) {
       origins.push({
         name: \`Agent Relay \${i}\`,
         latency: Math.floor(Math.random() * 100) + 20,
         status: Math.random() > 0.95 ? 'offline' : 'online'
       });
    }

    res.json({
      active_connections: 20,
      latency_ms: Math.floor(Math.random() * 40) + 10,
      req_per_sec: Math.floor(Math.random() * 500) + 1200,
      cpu_usage: Math.floor(Math.random() * 15) + 5,
      mem_usage: Math.floor(Math.random() * 20) + 80,
      origins
    });
  });
`;

serverCode = serverCode.replace(/app\.get\("\/rail-api\/observability"[\s\S]*?\/\/\s*Vite middleware/, dynamicObservability.trim() + "\n\n  // Vite middleware");

fs.writeFileSync('server.ts', serverCode);
