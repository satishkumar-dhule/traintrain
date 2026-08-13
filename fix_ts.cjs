const fs = require('fs');
let content = fs.readFileSync('server.ts', 'utf-8');

content = content.replace(
  "const station = (req.query.station || '').toUpperCase();",
  "const station = ((req.query.station as string) || '').toUpperCase();"
).replace(
  "const src = (req.query.src || '').toUpperCase();",
  "const src = ((req.query.src as string) || '').toUpperCase();"
).replace(
  "const dst = (req.query.dst || '').toUpperCase();",
  "const dst = ((req.query.dst as string) || '').toUpperCase();"
).replace(
  "const type = (req.query.type || 'cancelled').toLowerCase();",
  "const type = ((req.query.type as string) || 'cancelled').toLowerCase();"
);

fs.writeFileSync('server.ts', content);
