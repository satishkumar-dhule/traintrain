const fs = require('fs');
let content = fs.readFileSync('src/modules/pnr/sources/irctc.source.ts', 'utf-8');

content = content.replace(
  'https://via.placeholder.com/150x50/e2e8f0/1e293b?text=W7K9G',
  'https://placehold.co/150x50/e2e8f0/1e293b.png?text=W7K9G'
);
content = content.replace(
  'https://via.placeholder.com/150x50/f87171/991b1b?text=X2M4P',
  'https://placehold.co/150x50/f87171/991b1b.png?text=X2M4P'
);

fs.writeFileSync('src/modules/pnr/sources/irctc.source.ts', content);
