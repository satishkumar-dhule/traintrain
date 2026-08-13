export interface PnrResponse {
  pnr?: string;
  train_number?: string;
  train_name?: string;
  journey_date?: string;
  from?: { code: string; name: string; time: string; day: number };
  to?: { code: string; name: string; time: string; day: number };
  passengers?: Array<{ booking_status: string; coach: string; berth: string; current_status: string }>;
  last_updated?: string;
  freshness?: string;
  notice?: string;
  error?: string;
}

export interface ScheduleResponse {
  train_number?: string;
  train_name?: string;
  route_description?: string;
  running_days?: string[];
  stops?: Array<{ code: string; name: string; arrival: string; departure: string; day: number }>;
  source?: string;
  cache_ttl?: number;
  notice?: string;
  error?: string;
}

export interface StationResponse {
  code: string;
  name: string;
  city: string;
  zone: string;
}

export interface SourceStatusResponse {
  live_enabled: boolean;
  mode: string;
  cache_ttl_seconds: number;
  primary_source: string;
  verification_links: string[];
  notice: string;
}
