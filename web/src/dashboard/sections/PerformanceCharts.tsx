import {
  ResponsiveContainer, AreaChart, Area, BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, Cell,
} from "recharts";
import { DashStat } from "../DashStat";
import { analytics24h, latencyBuckets, requestsToday, cacheHitRate } from "../../data/realData";

// Performance overview — request-volume area chart + latency bars.

const BAR_COLORS = ["#383534", "#7B6BC9", "#C5C4FF", "#CFB8FF", "#FFDFC4"];

const axis = { fontSize: 10, fontFamily: "'Space Mono', monospace", fill: "#39322d" };
const tooltipStyle = {
  background: "#383534", border: "none", borderRadius: 10, color: "#FFDFC4",
  fontFamily: "'Space Mono', monospace", fontSize: 12, padding: "8px 12px",
};

export function PerformanceCharts() {
  return (
    <section
      className="dash-section dash-perf"
      style={{ background: "#E8A5F3", color: "#070707" }}
      data-text="#070707"
      data-label="AGENTSPAN // PERFORMANCE"
    >
      <div className="dash-section-head">
        <div>
          <span className="eyebrow mono">[ LAST 24 HOURS ]</span>
          <h2 className="dash-h">PERFORMANCE</h2>
        </div>
      </div>

      <div className="chart-grid">
        <div className="chart-card">
          <span className="chart-title mono">REQUEST VOLUME · 24H</span>
          <ResponsiveContainer width="100%" height={230}>
            <AreaChart data={analytics24h} margin={{ top: 8, right: 6, left: -18, bottom: 0 }}>
              <defs>
                <linearGradient id="vol-fill" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor="#9b4bd6" stopOpacity={0.55} />
                  <stop offset="100%" stopColor="#9b4bd6" stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid stroke="rgba(57,50,45,0.14)" vertical={false} />
              <XAxis dataKey="hour" interval={3} tick={axis} tickLine={false} axisLine={false} />
              <YAxis tick={axis} tickLine={false} axisLine={false} width={42} />
              <Tooltip contentStyle={tooltipStyle} cursor={{ stroke: "#39322d", strokeWidth: 1 }} />
              <Area type="monotone" dataKey="requests" stroke="#5a2a8a" strokeWidth={2} fill="url(#vol-fill)" />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        <div className="chart-card">
          <span className="chart-title mono">LATENCY DISTRIBUTION · MS</span>
          <ResponsiveContainer width="100%" height={230}>
            <BarChart data={latencyBuckets} margin={{ top: 8, right: 6, left: -18, bottom: 0 }}>
              <CartesianGrid stroke="rgba(57,50,45,0.14)" vertical={false} />
              <XAxis dataKey="range" tick={axis} tickLine={false} axisLine={false} />
              <YAxis tick={axis} tickLine={false} axisLine={false} width={42} allowDecimals={false} />
              <Tooltip contentStyle={tooltipStyle} cursor={{ fill: "rgba(57,50,45,0.08)" }} />
              <Bar dataKey="count" radius={[6, 6, 0, 0]}>
                {latencyBuckets.map((_, i) => (
                  <Cell key={i} fill={BAR_COLORS[i % BAR_COLORS.length]} />
                ))}
              </Bar>
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>

      <div className="dash-stat-row is-mini-row">
        <DashStat label="Requests Today" value={requestsToday} mini />
        <DashStat label="Cache Hit Rate" value={cacheHitRate} decimals={1} suffix="%" mini />
        <DashStat label="Error Rate" value={0.8} decimals={1} suffix="%" mini />
      </div>
    </section>
  );
}
