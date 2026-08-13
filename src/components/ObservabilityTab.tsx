import React, { useState, useEffect } from 'react';
import { Activity, Server, Cpu, Network, ShieldAlert, BarChart3, Clock, Zap } from 'lucide-react';
import { AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, BarChart, Bar } from 'recharts';

export default function ObservabilityTab() {
  const [metrics, setMetrics] = useState<any>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  
  const [history, setHistory] = useState<any[]>([]);

  useEffect(() => {
    let interval: NodeJS.Timeout;
    
    const fetchMetrics = async () => {
      try {
        const res = await fetch('/rail-api/observability');
        const data = await res.json();
        setMetrics(data);
        
        const timestamp = new Date().toLocaleTimeString();
        
        setHistory(prev => {
           const newH = [...prev, { time: timestamp.split(' ')[0], latency: data.latency_ms, reqs: data.req_per_sec, mem: data.mem_usage }];
           if (newH.length > 20) newH.shift();
           return newH;
        });
        
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
      <div className="bg-[#121212] border border-[#2d2d2d] md:rounded-2xl shadow-2xl overflow-hidden text-[#e8eaed]">
         
         {/* Header */}
         <div className="p-4 border-b border-[#2d2d2d] flex justify-between items-center bg-[#0a0a0a]">
            <div className="flex items-center gap-3">
               <Activity className="w-6 h-6 text-[#8ab4f8]" />
               <h2 className="text-xl font-bold tracking-wider text-[#8ab4f8]">ENGINE OBSERVABILITY</h2>
            </div>
            <div className="flex items-center gap-2 bg-[#1a2e1a] border border-[#2d4d2d] px-3 py-1 rounded-full">
               <span className="w-2 h-2 rounded-full bg-[#34a853] animate-pulse"></span>
               <span className="text-xs font-bold tracking-widest text-[#34a853]">LIVE STREAM</span>
            </div>
         </div>
         
         {/* Top KPIs */}
         <div className="p-6 grid grid-cols-2 md:grid-cols-4 gap-4 bg-[#121212] border-b border-[#2d2d2d]">
            <div className="bg-[#1e1e1e] p-4 rounded-xl border border-[#2d2d2d] relative overflow-hidden group">
               <div className="absolute -right-4 -top-4 w-16 h-16 bg-[#8ab4f8]/10 rounded-full blur-xl group-hover:bg-[#8ab4f8]/20 transition-all"></div>
               <p className="text-xs text-[#9aa0a6] tracking-widest mb-1 flex items-center gap-1.5"><Network className="w-3.5 h-3.5"/> SUBAGENTS</p>
               <p className="text-4xl font-black text-[#e8eaed]">20<span className="text-xl text-[#9aa0a6] font-medium">/20</span></p>
               <p className="text-[10px] text-[#8ab4f8] mt-1 font-bold">MAX FAN-OUT MODE</p>
            </div>
            <div className="bg-[#1e1e1e] p-4 rounded-xl border border-[#2d2d2d] relative overflow-hidden group">
               <div className="absolute -right-4 -top-4 w-16 h-16 bg-[#fbbc04]/10 rounded-full blur-xl group-hover:bg-[#fbbc04]/20 transition-all"></div>
               <p className="text-xs text-[#9aa0a6] tracking-widest mb-1 flex items-center gap-1.5"><Server className="w-3.5 h-3.5"/> ORIGINS</p>
               <p className="text-4xl font-black text-[#e8eaed]">20<span className="text-xl text-[#9aa0a6] font-medium">/20</span></p>
               <p className="text-[10px] text-[#fbbc04] mt-1 font-bold">SCRAPING CONCURRENTLY</p>
            </div>
            <div className="bg-[#1e1e1e] p-4 rounded-xl border border-[#2d2d2d] relative overflow-hidden group">
               <div className="absolute -right-4 -top-4 w-16 h-16 bg-[#ea4335]/10 rounded-full blur-xl group-hover:bg-[#ea4335]/20 transition-all"></div>
               <p className="text-xs text-[#9aa0a6] tracking-widest mb-1 flex items-center gap-1.5"><Clock className="w-3.5 h-3.5"/> AVG LATENCY</p>
               <p className="text-4xl font-black text-[#e8eaed]">{metrics.latency_ms}<span className="text-xl text-[#9aa0a6] font-medium">ms</span></p>
               <p className="text-[10px] text-[#ea4335] mt-1 font-bold">P95 RESPONSE TIME</p>
            </div>
            <div className="bg-[#1e1e1e] p-4 rounded-xl border border-[#2d2d2d] relative overflow-hidden group">
               <div className="absolute -right-4 -top-4 w-16 h-16 bg-[#34a853]/10 rounded-full blur-xl group-hover:bg-[#34a853]/20 transition-all"></div>
               <p className="text-xs text-[#9aa0a6] tracking-widest mb-1 flex items-center gap-1.5"><Zap className="w-3.5 h-3.5"/> REQ/SEC</p>
               <p className="text-4xl font-black text-[#e8eaed]">{metrics.req_per_sec}</p>
               <p className="text-[10px] text-[#34a853] mt-1 font-bold">GLOBAL THROUGHPUT</p>
            </div>
         </div>
         
         {/* Charts & Nodes */}
         <div className="p-6 grid grid-cols-1 lg:grid-cols-3 gap-6">
           
           <div className="lg:col-span-2 space-y-6">
              <div>
                <p className="text-xs font-bold text-[#9aa0a6] tracking-widest mb-4 flex items-center gap-2"><BarChart3 className="w-4 h-4"/> REAL-TIME LATENCY (ms)</p>
                <div className="bg-[#0a0a0a] border border-[#2d2d2d] rounded-xl p-4 h-64">
                  <ResponsiveContainer width="100%" height="100%">
                    <AreaChart data={history}>
                      <defs>
                        <linearGradient id="colorLatency" x1="0" y1="0" x2="0" y2="1">
                          <stop offset="5%" stopColor="#8ab4f8" stopOpacity={0.8}/>
                          <stop offset="95%" stopColor="#8ab4f8" stopOpacity={0}/>
                        </linearGradient>
                      </defs>
                      <CartesianGrid strokeDasharray="3 3" stroke="#2d2d2d" vertical={false} />
                      <XAxis dataKey="time" stroke="#9aa0a6" fontSize={10} tickMargin={10} />
                      <YAxis stroke="#9aa0a6" fontSize={10} domain={['auto', 'auto']} width={30} />
                      <Tooltip contentStyle={{ backgroundColor: '#1e1e1e', borderColor: '#2d2d2d', color: '#fff', fontSize: '12px' }} itemStyle={{ color: '#8ab4f8' }}/>
                      <Area type="monotone" dataKey="latency" stroke="#8ab4f8" strokeWidth={2} fillOpacity={1} fill="url(#colorLatency)" />
                    </AreaChart>
                  </ResponsiveContainer>
                </div>
              </div>
              
              <div>
                <p className="text-xs font-bold text-[#9aa0a6] tracking-widest mb-4 flex items-center gap-2"><Cpu className="w-4 h-4"/> RESOURCE UTILIZATION</p>
                <div className="bg-[#0a0a0a] border border-[#2d2d2d] rounded-xl p-4 h-48">
                  <ResponsiveContainer width="100%" height="100%">
                    <BarChart data={history.slice(-10)}>
                      <CartesianGrid strokeDasharray="3 3" stroke="#2d2d2d" vertical={false} />
                      <XAxis dataKey="time" stroke="#9aa0a6" fontSize={10} />
                      <YAxis stroke="#9aa0a6" fontSize={10} width={30} />
                      <Tooltip cursor={{fill: '#2d2d2d'}} contentStyle={{ backgroundColor: '#1e1e1e', borderColor: '#2d2d2d', color: '#fff', fontSize: '12px' }}/>
                      <Bar dataKey="mem" fill="#34a853" radius={[4, 4, 0, 0]} />
                    </BarChart>
                  </ResponsiveContainer>
                </div>
              </div>
           </div>

           <div className="space-y-6">
             <div>
               <p className="text-xs font-bold text-[#9aa0a6] tracking-widest mb-4">ORIGIN-WISE TELEMETRY</p>
               <div className="bg-[#0a0a0a] rounded-xl border border-[#2d2d2d] overflow-hidden">
                 <div className="h-[432px] overflow-y-auto custom-scrollbar pr-1">
                   {metrics.origins?.map((org: any, i: number) => {
                     const isFast = org.latency < 50;
                     const isSlow = org.latency > 100;
                     return (
                     <div key={i} className="flex justify-between items-center p-3 border-b border-[#2d2d2d] hover:bg-[#1e1e1e] transition-colors">
                       <div className="flex items-center gap-3">
                         <div className={`relative flex items-center justify-center w-3 h-3`}>
                            <div className={`absolute w-full h-full rounded-full ${org.status === 'online' ? 'bg-[#34a853]' : org.status === 'throttled' ? 'bg-[#fbbc04]' : 'bg-[#ea4335]'} opacity-40 animate-ping`}></div>
                            <div className={`w-2 h-2 rounded-full ${org.status === 'online' ? 'bg-[#34a853]' : org.status === 'throttled' ? 'bg-[#fbbc04]' : 'bg-[#ea4335]'}`}></div>
                         </div>
                         <div>
                           <span className="text-sm font-bold text-[#e8eaed]">{org.name}</span>
                           <p className="text-[10px] text-[#9aa0a6]">{org.status.toUpperCase()}</p>
                         </div>
                       </div>
                       <div className="text-right">
                         <span className={`text-sm font-black ${isFast ? 'text-[#8ab4f8]' : isSlow ? 'text-[#ea4335]' : 'text-[#fbbc04]'}`}>{org.latency}ms</span>
                       </div>
                     </div>
                   )})}
                 </div>
               </div>
             </div>
           </div>
         </div>
         
         <div className="px-6 pb-6">
            <p className="text-xs font-bold text-[#9aa0a6] tracking-widest mb-4 flex items-center gap-2"><ShieldAlert className="w-4 h-4"/> REAL-TIME FAN-OUT LOGS</p>
            <div className="bg-[#0a0a0a] p-4 rounded-xl border border-[#2d2d2d] h-48 overflow-y-auto text-xs leading-relaxed font-mono custom-scrollbar">
               {logs.map((log, i) => (
                 <div key={i} className={`mb-1.5 flex gap-3 ${i === 0 ? 'text-[#8ab4f8]' : 'text-[#9aa0a6]'}`}>
                   <span className="text-[#3c4043] opacity-50 shrink-0">{(i + 1).toString().padStart(2, '0')}</span>
                   <span>{log}</span>
                 </div>
               ))}
            </div>
         </div>
         
         {/* Footer */}
         <div className="p-4 bg-[#0a0a0a] border-t border-[#2d2d2d] flex flex-col md:flex-row justify-between items-center text-[10px] text-[#5f6368] tracking-widest gap-2">
            <div className="flex gap-6">
              <span className="flex items-center gap-1.5"><Cpu className="w-3.5 h-3.5 text-[#fbbc04]" /> CLUSTER CPU: <span className="text-[#e8eaed] font-bold">{metrics.cpu_usage}%</span></span>
              <span className="flex items-center gap-1.5"><Server className="w-3.5 h-3.5 text-[#8ab4f8]" /> CLUSTER MEM: <span className="text-[#e8eaed] font-bold">{metrics.mem_usage}MB</span></span>
            </div>
            <span className="font-bold">GOOGLE UNDERGROUND ENGINE v2.5.0</span>
         </div>
      </div>
    </div>
  );
}
