import React, { useState, useEffect } from 'react';
import { Settings, Zap, Database, Shield, Monitor, Moon, Contrast } from 'lucide-react';

export default function SettingsTab() {
  const [maxMode, setMaxMode] = useState(true);
  const [cacheEnabled, setCacheEnabled] = useState(false);
  const [invertColors, setInvertColors] = useState(() => {
    return document.documentElement.classList.contains('invert-colors');
  });

  useEffect(() => {
    if (invertColors) {
      document.documentElement.classList.add('invert-colors');
    } else {
      document.documentElement.classList.remove('invert-colors');
    }
  }, [invertColors]);

  return (
    <div className="w-full space-y-6 animate-in fade-in duration-500">
      <div className="bg-white border-b border-slate-200 md:border md:rounded-2xl shadow-sm p-6 sticky top-0 md:relative z-40">
         <div className="flex items-center gap-3 text-slate-900">
            <Settings className="w-8 h-8 text-blue-600" />
            <h2 className="text-2xl font-black tracking-tight">System Settings</h2>
         </div>
         <p className="text-slate-500 font-medium mt-1">Configure Google Underground Engine & Engine Parameters</p>
      </div>

      <div className="bg-white border-y md:border border-slate-200 md:rounded-2xl overflow-hidden shadow-sm p-6 space-y-8">
        
        {/* Invert Colors Toggle */}
        <div className="flex items-center justify-between">
          <div className="flex gap-4 items-start">
             <div className={`p-3 rounded-xl flex-shrink-0 ${invertColors ? 'bg-purple-100 text-purple-600' : 'bg-slate-100 text-slate-500'}`}>
                <Contrast className="w-6 h-6" />
             </div>
             <div>
                <h4 className="font-bold text-slate-900 text-lg">Invert Colors (Dark Pattern)</h4>
                <p className="text-slate-500 text-sm mt-1 max-w-md">Applies a global CSS inversion to match deep dark patterns while retaining structural layout.</p>
             </div>
          </div>
          <button 
            onClick={() => setInvertColors(!invertColors)} 
            className={`w-14 h-8 rounded-full transition-colors relative flex-shrink-0 ${invertColors ? 'bg-purple-600' : 'bg-slate-300'}`}
          >
            <div className={`absolute top-1 left-1 w-6 h-6 rounded-full bg-white transition-transform ${invertColors ? 'translate-x-6' : 'translate-x-0'}`}></div>
          </button>
        </div>

        <div className="w-full h-px bg-slate-100"></div>

        <div className="flex items-center justify-between">
          <div className="flex gap-4 items-start">
             <div className={`p-3 rounded-xl flex-shrink-0 ${maxMode ? 'bg-amber-100 text-amber-600' : 'bg-slate-100 text-slate-500'}`}>
                <Zap className="w-6 h-6" />
             </div>
             <div>
                <h4 className="font-bold text-slate-900 text-lg">Max Mode (Aggressive Fan-Out)</h4>
                <p className="text-slate-500 text-sm mt-1 max-w-md">Deploys the maximum 20-subagent team concurrently for every search. Increases data freshness but may consume higher bandwidth.</p>
             </div>
          </div>
          <button 
            onClick={() => setMaxMode(!maxMode)} 
            className={`w-14 h-8 rounded-full transition-colors relative flex-shrink-0 ${maxMode ? 'bg-amber-500' : 'bg-slate-300'}`}
          >
            <div className={`absolute top-1 left-1 w-6 h-6 rounded-full bg-white transition-transform ${maxMode ? 'translate-x-6' : 'translate-x-0'}`}></div>
          </button>
        </div>

        <div className="w-full h-px bg-slate-100"></div>

        <div className="flex items-center justify-between">
          <div className="flex gap-4 items-start">
             <div className="p-3 rounded-xl bg-blue-100 text-blue-600 flex-shrink-0">
                <Database className="w-6 h-6" />
             </div>
             <div>
                <h4 className="font-bold text-slate-900 text-lg">Local Cache Bypass</h4>
                <p className="text-slate-500 text-sm mt-1 max-w-md">Forces all requests to skip local proxy caches and hit direct origin sources via the underground engine.</p>
             </div>
          </div>
          <button 
            onClick={() => setCacheEnabled(!cacheEnabled)} 
            className={`w-14 h-8 rounded-full transition-colors relative flex-shrink-0 ${!cacheEnabled ? 'bg-blue-600' : 'bg-slate-300'}`}
          >
            <div className={`absolute top-1 left-1 w-6 h-6 rounded-full bg-white transition-transform ${!cacheEnabled ? 'translate-x-6' : 'translate-x-0'}`}></div>
          </button>
        </div>
      </div>
    </div>
  );
}
