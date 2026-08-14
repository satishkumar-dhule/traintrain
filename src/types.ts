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
  data_source?: string;
  error?: string;
}

export interface ScheduleResponse {
  train_number?: string;
  train_name?: string;
  route_description?: string;
  running_days?: string[];
  stops?: Array<{ code: string; name: string; arrival: string; departure: string; day: number }>;
  source?: string;
  data_source?: string;
  cache_ttl?: number;
  notice?: string;
  error?: string;
}

export interface LiveStatusResponse {
  train_number?: string;
  train_name?: string;
  start_date?: string;
  update_time?: string;
  status_as_of?: string;
  is_run_day?: boolean;
  current_location_info?: string;
  current_station?: {
    code: string;
    name: string;
    status: string;
    delay_minutes: number;
    platform_number: string;
    eta: string;
    etd: string;
  };
  next_stoppage?: { name: string; in: string; delay_minutes: number } | null;
  journey?: {
    from: { code: string; name: string };
    to: { code: string; name: string };
    std: string;
    run_days: string;
    distance_from_source: number;
    total_distance: number;
    avg_speed: number;
  };
  upcoming_stations?: Array<{
    code: string;
    name: string;
    eta: string;
    etd: string;
    day: number;
    delay_minutes: number;
    platform: string;
  }>;
  previous_stations?: Array<{
    code: string;
    name: string;
    eta: string;
    etd: string;
    day: number;
    delay_minutes: number;
    platform: string;
  }>;
  source?: string;
  data_source?: string;
  notice?: string;
  error?: string;
}

export interface TrainsBetweenResponse {
  src?: string;
  dst?: string;
  date?: string;
  train_count?: number;
  trains?: Array<{
    number: string;
    name: string;
    departure_time: string;
    arrival_time: string;
    duration: string;
    duration_min: number;
    distance: number;
    from_station: string;
    to_station: string;
    runs_on: boolean[];
    classes: string[];
  }>;
  source?: string;
  data_source?: string;
  notice?: string;
  error?: string;
}

export interface LiveStationResponse {
  station?: string;
  hours?: number;
  trains?: Array<{ number: string; name: string; eta: string; delay_arr: boolean; platform: string }>;
  data_source?: string;
  error?: string;
  code?: string;
}

export interface ExceptionalResponse {
  type?: string;
  trains?: Array<{ number: string; name: string; date: string; reason: string }>;
  data_source?: string;
  error?: string;
  code?: string;
}

export interface ObservabilityResponse {
  process?: {
    uptime_seconds: number;
    node_version: string;
    pid: number;
    memory: { rss_mb: number; heap_used_mb: number; heap_total_mb: number };
  };
  traffic?: {
    requests_last_5s: number;
    req_per_sec: number;
    total_requests: number;
  };
  sources?: Array<{ name: string; status: string; latency_ms: number; error?: string }>;
  notice?: string;
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
