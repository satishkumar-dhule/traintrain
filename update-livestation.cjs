const fs = require('fs');

let code = fs.readFileSync('src/components/LiveStationTab.tsx', 'utf-8');

if (!code.includes('Server')) {
  code = code.replace(/import {([^}]+)} from 'lucide-react';/, "import {$1, Server} from 'lucide-react';");
}

const sourceBadge = `               <div>
                  <h2 className="text-2xl sm:text-3xl font-black uppercase tracking-widest">{station} DEPARTURES</h2>
                  <div className="flex items-center gap-1.5 mt-1">
                     <Server className="w-3 h-3 text-zinc-500" />
                     <span className="text-[10px] font-bold text-zinc-500 tracking-widest uppercase">Origin: <span className="text-blue-400">{data.data_source || 'Cache'}</span></span>
                  </div>
               </div>
               <p className="text-sm font-bold text-zinc-400">NEXT {hours}H</p>`;

code = code.replace(/<h2 className="text-2xl sm:text-3xl font-black uppercase tracking-widest">\{station\} DEPARTURES<\/h2>\s*<p className="text-sm font-bold text-zinc-400">NEXT \{hours\}H<\/p>/, sourceBadge);

fs.writeFileSync('src/components/LiveStationTab.tsx', code);
