const fs = require('fs');

let code = fs.readFileSync('src/components/LiveStatusTab.tsx', 'utf-8');

const targetStr = `                 <div className="text-right">
                   <p className="text-sm font-bold text-slate-400">Journey Date</p>
                   <p className="text-lg font-bold text-emerald-400">{date}</p>
               </div>
               <div className="mt-3 bg-slate-800 border border-slate-700 p-2.5 rounded-lg flex justify-between items-center">
                  <p className="text-xs font-bold text-slate-400">Data provided by Google Underground Engine</p>
                  <div className="flex items-center gap-1.5">
                     <Server className="w-3.5 h-3.5 text-emerald-400" />
                     <span className="text-xs font-bold text-slate-300">Origin: <span className="text-emerald-400">{data.data_source || 'Cache'}</span></span>
                  </div>
               </div>
            </div>
               <div className="mt-4 bg-blue-600/20 border border-blue-500/30 p-4 rounded-xl flex items-start gap-3">
                  <Navigation className="w-5 h-5 text-blue-400 flex-shrink-0 mt-0.5" />
                  <p className="font-bold text-blue-100 text-sm">{data.current_location_info}</p>
               </div>
            </div>`;

const correctStr = `                 <div className="text-right">
                   <p className="text-sm font-bold text-slate-400">Journey Date</p>
                   <p className="text-lg font-bold text-emerald-400">{date}</p>
                 </div>
               </div>
               
               <div className="mt-4 bg-blue-600/20 border border-blue-500/30 p-4 rounded-xl flex items-start gap-3">
                  <Navigation className="w-5 h-5 text-blue-400 flex-shrink-0 mt-0.5" />
                  <p className="font-bold text-blue-100 text-sm">{data.current_location_info}</p>
               </div>
               
               <div className="mt-3 bg-slate-800 border border-slate-700 p-2.5 rounded-lg flex justify-between items-center">
                  <p className="text-xs font-bold text-slate-400">Data provided by Google Underground Engine</p>
                  <div className="flex items-center gap-1.5">
                     <Server className="w-3.5 h-3.5 text-emerald-400" />
                     <span className="text-xs font-bold text-slate-300">Origin: <span className="text-emerald-400">{data.data_source || 'Cache'}</span></span>
                  </div>
               </div>
            </div>`;

code = code.replace(targetStr, correctStr);

fs.writeFileSync('src/components/LiveStatusTab.tsx', code);
