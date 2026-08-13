const fs = require('fs');
let content = fs.readFileSync('src/App.tsx', 'utf-8');
// Fix the CSS class list so it transitions smoothly when inverted
content = content.replace("min-h-screen bg-slate-50 flex flex-col md:flex-row text-slate-900 font-sans sm:overflow-hidden", "min-h-screen bg-slate-50 flex flex-col md:flex-row text-slate-900 font-sans sm:overflow-hidden transition-colors duration-500");
fs.writeFileSync('src/App.tsx', content);
