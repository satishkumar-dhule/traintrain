const fs = require('fs');

let code = fs.readFileSync('src/components/TrainsBetweenTab.tsx', 'utf-8');

if (!code.includes('Server')) {
  code = code.replace(/import {([^}]+)} from 'lucide-react';/, "import {$1, Server} from 'lucide-react';");
}

const sourceBadge = `<div className="bg-slate-50 p-4 border-b border-slate-200 flex justify-between items-center">
               <h3 className="font-bold text-slate-700 uppercase tracking-widest text-xs">Available Trains</h3>
               <div className="flex items-center gap-1.5">
                  <Server className="w-3.5 h-3.5 text-blue-600" />
                  <span className="text-xs font-bold text-slate-500">Origin: <span className="text-blue-600">{data.data_source || 'Cache'}</span></span>
               </div>
             </div>
             {data.trains?.map`;

code = code.replace(/\{data\.trains\?\.map/, sourceBadge);

fs.writeFileSync('src/components/TrainsBetweenTab.tsx', code);
