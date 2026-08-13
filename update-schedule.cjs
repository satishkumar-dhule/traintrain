const fs = require('fs');

let code = fs.readFileSync('src/components/ScheduleTab.tsx', 'utf-8');

if (!code.includes('Server')) {
  code = code.replace(/import {([^}]+)} from 'lucide-react';/, "import {$1, Server} from 'lucide-react';");
}

const sourceBadge = `                <p className="text-xs text-amber-600 bg-amber-50 px-2 py-1 rounded font-medium">
                  {data.notice}
                </p>
                <div className="flex items-center gap-1.5 mt-2 bg-slate-200 px-2.5 py-1 rounded-md">
                   <Server className="w-3.5 h-3.5 text-slate-600" />
                   <span className="text-xs font-bold text-slate-600">Source: <span className="text-blue-600">{data.data_source || 'Cache'}</span></span>
                </div>`;

code = code.replace(/<p className="text-xs text-amber-600 bg-amber-50 px-2 py-1 rounded font-medium">\s*\{data\.notice\}\s*<\/p>/, sourceBadge);

fs.writeFileSync('src/components/ScheduleTab.tsx', code);
