const fs = require('fs');
let content = fs.readFileSync('server.ts', 'utf-8');
content = content.replace(
  `  // Add a simple request logger
  app.use((req, res, next) => {
    console.log(\`\${new Date().toISOString()} [API] \${req.method} \${req.url}\`);
    next();
  });`,
  `  // Logger removed to prevent console clutter`
);
fs.writeFileSync('server.ts', content);
