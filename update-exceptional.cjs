const fs = require('fs');

let code = fs.readFileSync('src/components/ExceptionalTrainsTab.tsx', 'utf-8');

if (!code.includes('Server')) {
  code = code.replace(/import {([^}]+)} from 'lucide-react';/, "import {$1, Server} from 'lucide-react';");
}

const sourceBadge = `<div className="p-4 bg-slate-900 text-white flex justify-between items-center">
                <div className="flex items-center gap-2">
                   <AlertTriangle className={\`w-5 h-5 \${type==='cancelled'?'text-red-500':type==='rescheduled'?'text-amber-500':'text-purple-500'}\`} />
                   <h3 className="font-bold uppercase tracking-widest">{type} TRAINS</h3>
                </div>
                <div className="flex items-center gap-1.5">
                   <Server className="w-3.5 h-3.5 text-slate-400" />
                   <span className="text-xs font-bold text-slate-400">Origin: <span className="text-white">{data.data_source || 'Cache'}</span></span>
                </div>
             </div>`;

code = code.replace(/<div className="p-4 bg-slate-900 text-white flex items-center gap-2">[\s\S]*?<\/div>/, sourceBadge);

fs.writeFileSync('src/components/ExceptionalTrainsTab.tsx', code);
