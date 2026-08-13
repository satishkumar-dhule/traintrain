const fs = require('fs');

let content = fs.readFileSync('src/App.tsx', 'utf-8');

// Add imports
if (!content.includes('SettingsTab')) {
  content = content.replace("import ExceptionalTrainsTab from './components/ExceptionalTrainsTab';", "import ExceptionalTrainsTab from './components/ExceptionalTrainsTab';\nimport SettingsTab from './components/SettingsTab';\nimport ObservabilityTab from './components/ObservabilityTab';");
}

if (!content.includes("import { Train, Ticket, Activity, MapPin, Clock, AlertTriangle, Menu")) {
    content = content.replace("import { Train, Ticket, Activity, MapPin, Clock, AlertTriangle, Menu } from 'lucide-react';", "import { Train, Ticket, Activity, MapPin, Clock, AlertTriangle, Menu, Settings, Monitor } from 'lucide-react';");
} else {
    content = content.replace("Menu }", "Menu, Settings, Monitor }");
}

// Update Tab type
content = content.replace("type Tab = 'pnr' | 'live_status' | 'live_station' | 'trains_between' | 'schedule' | 'exceptional' | 'stations';", "type Tab = 'pnr' | 'live_status' | 'live_station' | 'trains_between' | 'schedule' | 'exceptional' | 'stations' | 'settings' | 'observability';");

// Add desktop buttons
const desktopButtonsStr = `
            { id: 'exceptional', label: 'Exceptional', icon: AlertTriangle },
            { id: 'stations', label: 'Stations', icon: MapPin },
            { id: 'settings', label: 'Settings', icon: Settings },
            { id: 'observability', label: 'Observability', icon: Monitor }
`;
content = content.replace(/{ id: 'exceptional', label: 'Exceptional', icon: AlertTriangle },\s*{ id: 'stations', label: 'Stations', icon: MapPin }/g, desktopButtonsStr.trim());

// Add mobile buttons
const mobileButtonsStr = `
          <TabBtn id="trains_between" label="Trains" icon={MapPin} />
          <TabBtn id="settings" label="Settings" icon={Settings} />
          <TabBtn id="observability" label="Engine" icon={Monitor} />
`;
content = content.replace(/<TabBtn id="trains_between" label="Trains" icon=\{MapPin\} \/>\s*<TabBtn id="exceptional" label="Alerts" icon=\{AlertTriangle\} \/>/g, mobileButtonsStr.trim());

// Add Main Content Tabs
const mainContentTabs = `
          {activeTab === 'exceptional' && <ExceptionalTrainsTab />}
          {activeTab === 'settings' && <SettingsTab />}
          {activeTab === 'observability' && <ObservabilityTab />}
`;
content = content.replace(/\{activeTab === 'exceptional' && <ExceptionalTrainsTab \/>\}/g, mainContentTabs.trim());


fs.writeFileSync('src/App.tsx', content);
