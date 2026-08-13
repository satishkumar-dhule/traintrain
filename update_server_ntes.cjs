const fs = require('fs');
let content = fs.readFileSync('server.ts', 'utf-8');

const ntesRoutes = `
  app.get("/rail-api/ntes/live-station", (req, res) => {
    const station = (req.query.station || '').toUpperCase();
    const hours = req.query.hours || '2';
    
    // Mock response for Live Station
    res.json({
      station,
      hours,
      trains: [
        { number: '12951', name: 'RAJDHANI EXP', source: 'MMCT', dest: 'NDLS', sta: '16:30', eta: '16:35', delay_arr: true, platform: '1' },
        { number: '12004', name: 'SHATABDI EXP', source: 'NDLS', dest: 'LKO', sta: '17:00', eta: '17:00', delay_arr: false, platform: '4' },
        { number: '12229', name: 'LUCKNOW MAIL', source: 'LKO', dest: 'NDLS', sta: '18:15', eta: '18:50', delay_arr: true, platform: '3' }
      ]
    });
  });

  app.get("/rail-api/ntes/trains-between", (req, res) => {
    const src = (req.query.src || '').toUpperCase();
    const dst = (req.query.dst || '').toUpperCase();
    
    // Mock response for Trains Between Stations
    res.json({
      src,
      dst,
      trains: [
        { number: '12951', name: 'RAJDHANI EXP', departure_time: '17:00', arrival_time: '08:32', runs_on: [true, true, true, true, true, true, true] },
        { number: '12953', name: 'AK TEJAS RAJ EX', departure_time: '17:10', arrival_time: '09:43', runs_on: [true, false, true, false, true, false, false] },
        { number: '12909', name: 'GARIB RATH', departure_time: '17:30', arrival_time: '10:15', runs_on: [false, true, false, true, false, true, false] }
      ]
    });
  });

  app.get("/rail-api/ntes/exceptional", (req, res) => {
    const type = (req.query.type || 'cancelled').toLowerCase();
    
    // Mock response for Exceptional Trains
    const trains = [];
    if (type === 'cancelled') {
      trains.push({ number: '11005', name: 'CHALUKYA EXP', date: new Date().toLocaleDateString(), reason: 'Operational Reasons' });
      trains.push({ number: '12168', name: 'MANDUADIH EXP', date: new Date().toLocaleDateString(), reason: 'Track Maintenance' });
    } else if (type === 'rescheduled') {
      trains.push({ number: '12810', name: 'HOWRAH MAIL', date: new Date().toLocaleDateString(), reason: 'Late running of pairing train' });
    } else {
      trains.push({ number: '12626', name: 'KERALA EXPRESS', date: new Date().toLocaleDateString(), reason: 'Waterlogging at originating station' });
    }
    
    res.json({ type, trains });
  });
`;

if (!content.includes('/rail-api/ntes/live-station')) {
  content = content.replace(
    'app.get("/rail-api/stations", (req, res) => {',
    ntesRoutes + '\n  app.get("/rail-api/stations", (req, res) => {'
  );
  fs.writeFileSync('server.ts', content);
}
