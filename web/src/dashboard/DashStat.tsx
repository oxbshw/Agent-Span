import type { LucideIcon } from "lucide-react";
import { TrendingUp, TrendingDown } from "lucide-react";
import { BlinkingBlock } from "../components/BlinkingBlock";
import { useCountUp } from "./useCountUp";

// A stat card in the landing's design language (Space Grotesk / Space Mono,
// subtle bordered card, hover lift). The value counts up on viewport entry.

type Dot = "online" | "degraded" | "offline";
const DOT_COLOR: Record<Dot, string> = { online: "#2e9e5b", degraded: "#E8A5F3", offline: "#39322d" };

interface Props {
  label: string;
  value: number;
  suffix?: string;
  decimals?: number;
  dot?: Dot;
  trend?: "up" | "down";
  icon?: LucideIcon;
  mini?: boolean;
}

export function DashStat({ label, value, suffix = "", decimals = 0, dot, trend, icon: Icon, mini }: Props) {
  const { ref, shown } = useCountUp(value, decimals);
  const Trend = trend === "up" ? TrendingUp : trend === "down" ? TrendingDown : null;

  return (
    <div className={`dash-stat ${mini ? "is-mini" : ""}`}>
      <div className="dash-stat-top">
        <span className="dash-stat-label mono">{label}</span>
        {dot && <BlinkingBlock color={DOT_COLOR[dot]} size={9} blink={dot === "degraded"} />}
        {Icon && !dot && <Icon size={15} strokeWidth={1.7} />}
      </div>
      <div className="dash-stat-value">
        <span ref={ref}>{shown}</span>
        {suffix && <span className="dash-stat-suffix">{suffix}</span>}
        {Trend && <Trend size={16} className="dash-stat-trend" />}
      </div>
    </div>
  );
}
