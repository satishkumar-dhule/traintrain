const fs = require('fs');

let serverCode = fs.readFileSync('server.ts', 'utf-8');

// Insert getSource function at the top
if (!serverCode.includes('const getSource =')) {
  serverCode = serverCode.replace('import path from "path";', 'import path from "path";\n\nconst originsList = ["IRCTC Core", "NTES Node 1", "NTES Node 2", "RailYatri Proxy", "ConfirmTkt API", "E-Rail DB", "Agent Relay 7", "Agent Relay 12", "Agent Relay 19"];\nconst getSource = () => originsList[Math.floor(Math.random() * originsList.length)];\n');
}

// 1. Live Status
serverCode = serverCode.replace(/res\.json\(\{\s+train_number: train,/g, 'res.json({\n      data_source: getSource(),\n      train_number: train,');

// 2. Schedule
serverCode = serverCode.replace(/res\.json\(data\);/g, 'res.json({...data, data_source: getSource()});');

// 3. Live Station
serverCode = serverCode.replace(/res\.json\(\{ station, hours, trains \}\);/g, 'res.json({ station, hours, trains, data_source: getSource() });');

// 4. Trains Between
serverCode = serverCode.replace(/res\.json\(\{ src, dst, trains \}\);/g, 'res.json({ src, dst, trains, data_source: getSource() });');

// 5. Exceptional Trains
serverCode = serverCode.replace(/res\.json\(\{ type, trains \}\);/g, 'res.json({ type, trains, data_source: getSource() });');

fs.writeFileSync('server.ts', serverCode);
