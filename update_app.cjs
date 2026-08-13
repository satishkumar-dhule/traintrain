const fs = require('fs');

const appTsx = `import React, { useState, useEffect } from 'react';
import PnrTab from './components/PnrTab';
import ScheduleTab from './components/ScheduleTab';
import StationsTab from './components/StationsTab';
import LiveStatusTab from './components/LiveStatusTab';
import LiveStationTab from './components/LiveStationTab';
import TrainsBetweenTab from './components/TrainsBetweenTab';
import ExceptionalTrainsTab from './components/ExceptionalTrainsTab';
import { SourceStatusResponse } from './types';
import { Train, Ticket, Activity, MapPin, Clock, AlertTriangle, Menu } from 'lucide-react';

type Tab = 'pnr' | 'live_status' | 'live_station' | 'trains_between' | 'schedule' | 'exceptional' | 'stations';

export default function App() {
  const [activeTab, setActiveTab] = useState<Tab>('pnr');
  const [sourceStatus, setSourceStatus] = useState<SourceStatusResponse | null>(null);

  useEffect(() => {
    fetch('/rail-api/source-status')
      .then(res => res.json())
      .then(data => setSourceStatus(data))
      .catch(err => console.error(err));
  }, []);

  const TabBtn = ({ id, label, icon: Icon }: { id: Tab, label: string, icon: any }) => (
    <button
      onClick={() => setActiveTab(id)}
      className={\`flex-shrink-0 flex flex-col items-center justify-center w-20 py-2 gap-1 rounded-xl transition-colors \${
        activeTab === id ? 'text-blue-600 bg-blue-50/50 font-bold' : 'text-slate-500 font-medium'
      }\`}
    >
      <Icon className={\`w-5 h-5 \${activeTab === id ? 'stroke-[2.5px]' : 'stroke-2'}\`} />
      <span className="text-[10px] tracking-wide">{label}</span>
    </button>
  );

  return (
    <div className="min-h-screen bg-slate-50 flex flex-col md:flex-row text-slate-900 font-sans sm:overflow-hidden">
      
      {/* Mobile Header */}
      <header className="md:hidden bg-blue-600 text-white px-4 py-3 sticky top-0 z-50 shadow-md flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Train className="w-6 h-6" />
          <h1 className="text-xl font-bold tracking-tight">RailCompanion</h1>
        </div>
        {sourceStatus && (
          <div className="text-[10px] font-bold uppercase tracking-wider bg-black/20 px-2 py-1 rounded">
            {sourceStatus.mode}
          </div>
        )}
      </header>

      {/* Desktop Sidebar */}
      <aside className="hidden md:flex flex-col w-24 lg:w-64 bg-white border-r border-slate-200 h-screen sticky top-0 z-50">
        <div className="p-4 lg:p-6 flex items-center gap-3 text-blue-600 border-b border-slate-100">
          <Train className="w-8 h-8 flex-shrink-0" />
          <h1 className="text-xl font-bold tracking-tight hidden lg:block text-slate-900">RailCompanion</h1>
        </div>
        <nav className="flex-1 overflow-y-auto p-3 flex flex-col gap-1 hide-scrollbar">
          {[
            { id: 'pnr', label: 'PNR Status', icon: Ticket },
            { id: 'live_status', label: 'Spot Train', icon: Activity },
            { id: 'live_station', label: 'Live Station', icon: Clock },
            { id: 'trains_between', label: 'Trains B/W', icon: MapPin },
            { id: 'schedule', label: 'Schedule', icon: Train },
            { id: 'exceptional', label: 'Exceptional', icon: AlertTriangle },
            { id: 'stations', label: 'Stations', icon: MapPin }
          ].map(t => (
            <button
              key={t.id}
              onClick={() => setActiveTab(t.id as Tab)}
              className={\`flex items-center gap-3 p-3 lg:px-4 lg:py-3 rounded-xl transition-all \${
                activeTab === t.id ? 'bg-blue-600 text-white shadow-md' : 'text-slate-600 hover:bg-slate-100'
              }\`}
            >
              <t.icon className={\`w-6 h-6 flex-shrink-0 \${activeTab === t.id ? 'stroke-[2.5px]' : 'stroke-2'}\`} />
              <span className="font-bold text-sm hidden lg:block">{t.label}</span>
            </button>
          ))}
        </nav>
        {sourceStatus && (
          <div className="p-4 border-t border-slate-100 hidden lg:block">
             <div className="bg-slate-50 p-3 rounded-xl border border-slate-200">
                <p className="text-[10px] font-bold text-slate-400 uppercase">Data Mode</p>
                <p className="text-sm font-bold text-emerald-600 flex items-center gap-1">
                   <span className="w-2 h-2 rounded-full bg-emerald-500"></span> Live Integrated
                </p>
             </div>
          </div>
        )}
      </aside>

      {/* Main Content Area */}
      <main className="flex-1 w-full max-w-2xl lg:max-w-4xl mx-auto md:p-6 pb-24 md:pb-6 overflow-y-auto h-screen hide-scrollbar">
        <div className="min-h-full">
          {activeTab === 'pnr' && <PnrTab />}
          {activeTab === 'schedule' && <ScheduleTab />}
          {activeTab === 'stations' && <StationsTab />}
          {activeTab === 'live_status' && <LiveStatusTab />}
          {activeTab === 'live_station' && <LiveStationTab />}
          {activeTab === 'trains_between' && <TrainsBetweenTab />}
          {activeTab === 'exceptional' && <ExceptionalTrainsTab />}
        </div>
      </main>

      {/* Mobile Bottom Navigation */}
      <nav className="md:hidden fixed bottom-0 left-0 right-0 bg-white border-t border-slate-200 pb-safe z-50 shadow-[0_-4px_20px_rgba(0,0,0,0.05)]">
        <div className="flex overflow-x-auto hide-scrollbar px-2 py-1 items-center justify-between">
          <TabBtn id="pnr" label="PNR" icon={Ticket} />
          <TabBtn id="live_status" label="Spot" icon={Activity} />
          <TabBtn id="live_station" label="Station" icon={Clock} />
          <TabBtn id="trains_between" label="Trains" icon={MapPin} />
          <TabBtn id="exceptional" label="Alerts" icon={AlertTriangle} />
        </div>
      </nav>
    </div>
  );
}
`;

fs.writeFileSync('src/App.tsx', appTsx);
