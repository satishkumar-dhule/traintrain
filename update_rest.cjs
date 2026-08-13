const fs = require('fs');

const exceptionalTsx = `import React, { useState, useEffect } from 'react';
import { AlertCircle, AlertTriangle } from 'lucide-react';

export default function ExceptionalTrainsTab() {
  const [type, setType] = useState('cancelled');
  const [data, setData] = useState<any>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const fetchTrains = async () => {
      setLoading(true);
      try {
        const res = await fetch(\`/rail-api/ntes/exceptional?type=\${type}\`);
        setData(await res.json());
      } finally { setLoading(false); }
    };
    fetchTrains();
  }, [type]);

  return (
    <div className="w-full">
      <div className="bg-white border-b border-slate-200 md:border md:rounded-2xl shadow-sm p-2 sticky top-0 md:relative z-40">
        <div className="flex bg-slate-100 rounded-xl p-1">
           {['cancelled', 'rescheduled', 'diverted'].map(t => (
             <button key={t} onClick={() => setType(t)} className={\`flex-1 py-3 text-sm font-bold uppercase tracking-widest rounded-lg transition-colors \${type === t ? 'bg-white shadow text-slate-900' : 'text-slate-500'}\`}>
               {t.substring(0, 3)}
             </button>
           ))}
        </div>
      </div>
      <div className="mt-4 md:mt-6">
        {data && (
           <div className="bg-white border-y md:border md:rounded-2xl overflow-hidden shadow-sm">
             <div className="p-4 bg-slate-900 text-white flex items-center gap-2">
                <AlertTriangle className={\`w-5 h-5 \${type==='cancelled'?'text-red-500':type==='rescheduled'?'text-amber-500':'text-purple-500'}\`} />
                <h3 className="font-bold uppercase tracking-widest">{type} TRAINS</h3>
             </div>
             {data.trains?.map((t:any, i:number) => (
               <div key={i} className="p-4 border-b border-slate-100">
                  <div className="flex justify-between items-start">
                     <div>
                       <h4 className="font-bold text-slate-900"><span className="text-slate-400 mr-2">{t.number}</span>{t.name}</h4>
                       <p className="text-xs font-bold text-slate-400 mt-1">{t.date}</p>
                     </div>
                     <div className="text-right max-w-[120px]">
                       <span className={\`text-[10px] font-bold uppercase tracking-widest px-2 py-1 rounded \${type==='cancelled'?'bg-red-100 text-red-700':type==='rescheduled'?'bg-amber-100 text-amber-700':'bg-purple-100 text-purple-700'}\`}>{type}</span>
                       <p className="text-[10px] font-medium text-slate-500 mt-1 leading-tight">{t.reason}</p>
                     </div>
                  </div>
               </div>
             ))}
           </div>
        )}
      </div>
    </div>
  );
}
`;

fs.writeFileSync('src/components/ExceptionalTrainsTab.tsx', exceptionalTsx);
