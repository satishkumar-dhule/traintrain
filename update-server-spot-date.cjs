const fs = require('fs');

let serverCode = fs.readFileSync('server.ts', 'utf-8');

const dynamicLiveStatus = `
  app.get("/rail-api/live-status", async (req, res) => {
    const train = (req.query.train || '').trim();
    const dateStr = (req.query.date || '').trim();
    if (!train) return res.status(400).json({error: "Train number missing"});
    
    // Find train name from data if possible
    let trainName = "EXPRESS TRAIN";
    try {
      const trains = JSON.parse(fs.readFileSync('data/trains.json', 'utf-8'));
      const t = trains.find(x => x.number === train);
      if (t) trainName = t.name;
    } catch(e) {}
    
    const now = new Date();
    let reqDate = now;
    if (dateStr) {
       reqDate = new Date(dateStr);
    }
    
    // Check difference in days (approximate)
    const diffTime = Math.round((reqDate.getTime() - now.getTime()) / (1000 * 3600 * 24));
    
    // Generate stations
    const stations = [];
    const numStations = 8;
    for (let i = 0; i < numStations; i++) {
       let isPassed = false;
       if (diffTime < 0) {
         isPassed = true; // completely past
       } else if (diffTime > 0) {
         isPassed = false; // completely future
       } else {
         isPassed = i < 4; // today - running
       }
       
       const delay = isPassed ? (Math.random() > 0.5 ? Math.floor(Math.random()*15) : 0) : (Math.random() > 0.3 ? Math.floor(Math.random()*30) : 0);
       
       const baseTime = new Date(reqDate.getTime() + ((i + 1) * 45 * 60000));
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
    
    let locationInfo = "";
    if (diffTime < 0) locationInfo = "Journey completed. Arrived at destination.";
    else if (diffTime > 0) locationInfo = "Train yet to start from source.";
    else locationInfo = \`Departed from \${stations[3].name} and heading towards \${stations[4].name}. On time.\`;
    
    res.json({
      train_number: train,
      train_name: trainName,
      current_location_info: locationInfo,
      stations
    });
  });
`;

serverCode = serverCode.replace(/app\.get\("\/rail-api\/live-status"[\s\S]*?app\.get\("\/rail-api\/ntes\/live-station"/, dynamicLiveStatus.trim() + "\n\n  app.get(\"/rail-api/ntes/live-station\"");

fs.writeFileSync('server.ts', serverCode);
