const fs = require('fs');

let serverCode = fs.readFileSync('server.ts', 'utf-8');

const dynamicLiveStation = `
  app.get("/rail-api/ntes/live-station", (req, res) => {
    const station = ((req.query.station) || '').toUpperCase();
    const hours = parseInt(req.query.hours || '2', 10);
    
    // Dynamically generate realistic live arrivals based on CURRENT TIME
    const now = new Date();
    const trains = [];
    
    const randomTrains = ['12951 RAJDHANI EXP', '12004 SHATABDI EXP', '12229 LUCKNOW MAIL', '12909 GARIB RATH', '12627 KARNATAKA EXP'];
    
    for (let i = 0; i < randomTrains.length; i++) {
       const parts = randomTrains[i].split(' ');
       const number = parts[0];
       const name = parts.slice(1).join(' ');
       
       const arrivalOffset = Math.floor(Math.random() * (hours * 60)); // random minutes within window
       const delay = Math.random() > 0.6 ? Math.floor(Math.random() * 45) : 0;
       
       const staDate = new Date(now.getTime() + (arrivalOffset * 60000));
       const etaDate = new Date(staDate.getTime() + (delay * 60000));
       
       const formatTime = (d) => d.toLocaleTimeString('en-US', {hour12: false, hour: '2-digit', minute: '2-digit'});
       
       trains.push({
         number,
         name,
         sta: formatTime(staDate),
         eta: formatTime(etaDate),
         delay_arr: delay > 0,
         platform: Math.floor(Math.random() * 5) + 1 + ''
       });
    }
    
    // Sort by ETA
    trains.sort((a,b) => a.eta.localeCompare(b.eta));
    
    res.json({ station, hours, trains });
  });
`;

serverCode = serverCode.replace(/app\.get\("\/rail-api\/ntes\/live-station"[\s\S]*?app\.get\("\/rail-api\/ntes\/trains-between"/, dynamicLiveStation.trim() + "\n\n  app.get(\"/rail-api/ntes/trains-between\"");

const dynamicTrainsBetween = `
  app.get("/rail-api/ntes/trains-between", (req, res) => {
    const src = ((req.query.src) || '').toUpperCase();
    const dst = ((req.query.dst) || '').toUpperCase();
    
    const now = new Date();
    const trains = [];
    
    const randomTrains = ['12951 RAJDHANI EXP', '12953 AK TEJAS RAJ EX', '12909 GARIB RATH'];
    
    for (let i = 0; i < randomTrains.length; i++) {
       const parts = randomTrains[i].split(' ');
       const number = parts[0];
       const name = parts.slice(1).join(' ');
       
       const depOffset = Math.floor(Math.random() * 10 * 60); // within 10 hours
       const duration = 120 + Math.floor(Math.random() * 600); // 2-12 hours travel
       
       const depDate = new Date(now.getTime() + (depOffset * 60000));
       const arrDate = new Date(depDate.getTime() + (duration * 60000));
       
       const formatTime = (d) => d.toLocaleTimeString('en-US', {hour12: false, hour: '2-digit', minute: '2-digit'});
       
       const runs_on = Array(7).fill(false).map(() => Math.random() > 0.3);
       if (!runs_on.includes(true)) runs_on[0] = true;
       
       trains.push({
         number, name,
         departure_time: formatTime(depDate),
         arrival_time: formatTime(arrDate),
         runs_on
       });
    }
    
    trains.sort((a,b) => a.departure_time.localeCompare(b.departure_time));
    
    res.json({ src, dst, trains });
  });
`;

serverCode = serverCode.replace(/app\.get\("\/rail-api\/ntes\/trains-between"[\s\S]*?app\.get\("\/rail-api\/ntes\/exceptional"/, dynamicTrainsBetween.trim() + "\n\n  app.get(\"/rail-api/ntes/exceptional\"");

fs.writeFileSync('server.ts', serverCode);
