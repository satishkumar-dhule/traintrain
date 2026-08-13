const fs = require('fs');

const trainsBetweenTsx = `import React, { useState } from 'react';
import { ArrowRight, Search } from 'lucide-react';

export default function TrainsBetweenTab() {
  const [src, setSrc] = useState('');
  const [dst, setDst] = useState('');
  const [loading, setLoading] = useState(false);
  const [data, setData] = useState<any>(null);

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    try {
      const res = await fetch(\`/rail-api/ntes/trains-between?src=\${src}&dst=\${dst}\`);
      setData(await res.json());
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="w-full">
      <div className="bg-white border-b border-slate-200 md:border md:rounded-2xl shadow-sm p-4 sticky top-0 md:relative z-40">
        <form onSubmit={handleSearch} className="flex gap-2">
          <input type="text" value={src} onChange={e=>setSrc(e.target.value.toUpperCase())} placeholder="FROM" className="w-2/5 bg-slate-100 text-slate-900 px-4 py-3 rounded-xl font-bold text-lg uppercase" required />
          <div className="flex items-center justify-center text-slate-400"><ArrowRight className="w-4 h-4" /></div>
          <input type="text" value={dst} onChange={e=>setDst(e.target.value.toUpperCase())} placeholder="TO" className="w-2/5 bg-slate-100 text-slate-900 px-4 py-3 rounded-xl font-bold text-lg uppercase" required />
          <button type="submit" disabled={loading} className="flex-1 bg-blue-600 text-white rounded-xl font-bold">{loading ? '...' : 'Go'}</button>
        </form>
      </div>
      <div className="mt-4 md:mt-6">
        {data && (
          <div className="bg-white border-y md:border md:rounded-2xl overflow-hidden shadow-sm">
             {data.trains?.map((t:any, i:number) => (
                <div key={i} className="p-4 border-b border-slate-100">
                   <div className="flex justify-between items-start mb-2">
                      <div className="font-bold text-lg text-slate-900"><span className="text-slate-400 text-sm mr-2">{t.number}</span>{t.name}</div>
                   </div>
                   <div className="flex items-center justify-between mt-4 bg-slate-50 p-3 rounded-xl">
                      <div className="text-center w-1/3"><div className="text-xs font-bold text-slate-400">DEP</div><div className="font-black text-xl text-slate-900">{t.departure_time}</div></div>
                      <div className="w-px h-8 bg-slate-200"></div>
                      <div className="text-center w-1/3"><div className="text-xs font-bold text-slate-400">ARR</div><div className="font-black text-xl text-slate-900">{t.arrival_time}</div></div>
                   </div>
                   <div className="flex gap-1 mt-3 justify-center">
                     {['M','T','W','T','F','S','S'].map((d,idx) => (
                        <div key={idx} className={\`w-6 h-6 flex items-center justify-center rounded text-[10px] font-bold \${t.runs_on[idx] ? 'bg-blue-600 text-white' : 'bg-slate-100 text-slate-300'}\`}>{d}</div>
                     ))}
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

fs.writeFileSync('src/components/TrainsBetweenTab.tsx', trainsBetweenTsx);
