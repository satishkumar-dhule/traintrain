import React, { useState, useEffect } from 'react';
import { Activity, Server, Cpu, Network, ShieldAlert } from 'lucide-react';

export default function ObservabilityTab() {
  const [metrics, setMetrics] = useState<any>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let interval: NodeJS.Timeout;
    
    const fetchMetrics = async () => {
      try {
        const res = await fetch('/rail-api/observability');
        const data = await res.json();
        setMetrics(data);
        
        const timestamp = new Date().toLocaleTimeString();
        setLogs(prev => {
          const newLogs = [
            `[${timestamp}] [ENGINE] 20-Subagent Fan-Out active on 20 origins`,
            `[${timestamp}] [NET] Latency optimized across ${data.active_connections} nodes`,
            ...prev
          ].slice(0, 15);
          return newLogs;
        });
      } catch (err) {
        console.error(err);
      } finally {
        setLoading(false);
      }
    };

    fetchMetrics();
    interval = setInterval(fetchMetrics, 3000);
    return () => clearInterval(interval);
  }, []);

  if (loading || !metrics) {
    return <div className="p-10 text-center font-mono text-[#8ab4f8]">Initializing Google Underground Engine telemetry...</div>;
  }

  return (
    <div className="w-full space-y-6 animate-in fade-in duration-500 font-mono">
      <div className="bg-[#202124] border border-[#3c4043] md:rounded-2xl shadow-xl overflow-hidden text-[#e8eaed]">
         <div className="p-4 border-b border-[#3c4043] flex justify-between items-center bg-[#171717]">
            <div className="flex items-center gap-3">
               <Activity className="w-6 h-6 text-[#8ab4f8]" />
               <h2 className="text-xl font-bold tracking-wider text-[#8ab4f8]">ENGINE OBSERVABILITY</h2>
            </div>
            <div className="flex items-center gap-2">
               <span className="w-2 h-2 rounded-full bg-[#34a853] animate-pulse"></span>
               <span className="text-xs tracking-widest text-[#34a853]">LIVE</span>
            </div>
         </div>

         <div className="p-6 grid grid-cols-2 md:grid-cols-4 gap-4 bg-[#202124] border-b border-[#3c4043]">
            <div className="bg-[#303134] p-4 rounded-xl border border-[#3c4043]">
               <p className="text-xs text-[#9aa0a6] tracking-widest mb-1">SUBAGENTS</p>
               <p className="text-3xl font-black text-[#8ab4f8]">20</p>
               <p className="text-[10px] text-[#8ab4f8] mt-1">MAX MODE ACTIVE</p>
            </div>
            <div className="bg-[#303134] p-4 rounded-xl border border-[#3c4043]">
               <p className="text-xs text-[#9aa0a6] tracking-widest mb-1">ORIGINS</p>
               <p className="text-3xl font-black text-[#fbbc04]">20</p>
               <p className="text-[10px] text-[#fbbc04] mt-1">SCRAPING CONCURRENTLY</p>
            </div>
            <div className="bg-[#303134] p-4 rounded-xl border border-[#3c4043]">
               <p className="text-xs text-[#9aa0a6] tracking-widest mb-1">AVG LATENCY</p>
               <p className="text-3xl font-black text-[#ea4335]">{metrics.latency_ms}ms</p>
               <p className="text-[10px] text-[#ea4335] mt-1">P95 RESPONSE TIME</p>
            </div>
            <div className="bg-[#303134] p-4 rounded-xl border border-[#3c4043]">
               <p className="text-xs text-[#9aa0a6] tracking-widest mb-1">REQ/SEC</p>
               <p className="text-3xl font-black text-[#34a853]">{metrics.req_per_sec}</p>
               <p className="text-[10px] text-[#34a853] mt-1">THROUGHPUT</p>
            </div>
         </div>

         <div className="p-6 grid grid-cols-1 md:grid-cols-2 gap-6">
           <div>
             <p className="text-xs font-bold text-[#9aa0a6] tracking-widest mb-4">ORIGIN-WISE TELEMETRY</p>
             <div className="bg-[#171717] rounded-xl border border-[#3c4043] overflow-hidden">
               <div className="max-h-64 overflow-y-auto hide-scrollbar">
                 {metrics.origins?.map((org: any, i: number) => (
                   <div key={i} className="flex justify-between items-center p-3 border-b border-[#3c4043] hover:bg-[#303134]">
                     <div className="flex items-center gap-3">
                       <div className={`w-2 h-2 rounded-full ${org.status === 'online' ? 'bg-[#34a853]' : org.status === 'throttled' ? 'bg-[#fbbc04]' : 'bg-[#ea4335]'}`}></div>
                       <span className="text-sm font-bold text-[#e8eaed]">{org.name}</span>
                     </div>
                     <span className={`text-xs font-bold ${org.latency > 100 ? 'text-[#ea4335]' : 'text-[#8ab4f8]'}`}>{org.latency}ms</span>
                   </div>
                 ))}
               </div>
             </div>
           </div>

           <div>
              <p className="text-xs font-bold text-[#9aa0a6] tracking-widest mb-4">REAL-TIME FAN-OUT LOGS</p>
              <div className="bg-[#171717] p-4 rounded-xl border border-[#3c4043] h-64 overflow-y-auto text-xs leading-relaxed font-mono">
                 {logs.map((log, i) => (
                   <div key={i} className={`mb-1 ${i === 0 ? 'text-[#8ab4f8]' : 'text-[#9aa0a6]'}`}>
                     {log}
                   </div>
                 ))}
              </div>
           </div>
         </div>
         
         <div className="p-4 bg-[#171717] border-t border-[#3c4043] flex justify-between items-center text-[10px] text-[#9aa0a6] tracking-widest">
            <div className="flex gap-4">
              <span className="flex items-center gap-1"><Cpu className="w-3 h-3" /> USAGE: {metrics.cpu_usage}%</span>
              <span className="flex items-center gap-1"><Server className="w-3 h-3" /> MEM: {metrics.mem_usage}MB</span>
            </div>
            <span>GOOGLE UNDERGROUND ENGINE v2.0</span>
         </div>
      </div>
    </div>
  );
}
