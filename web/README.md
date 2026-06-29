# AgentSpan Web

React 18 + TypeScript + Vite marketing site and gateway dashboard.

## Structure

- `/` — marketing site (Hero, Channels, Network, Architecture, Features)
- `/status` — live gateway dashboard (channel grid, health table, performance charts)

## Tech stack

- React 18 + TypeScript
- Vite (build tool)
- GSAP + ScrollTrigger (animations)
- Lenis (smooth scroll)
- Recharts (dashboard charts)
- Pure CSS (no Tailwind/Bootstrap)

## Development

```bash
npm install
npm run dev        # http://localhost:5173
npm run build      # production build to dist/
npm run typecheck  # type checking only
```

## Docker

```bash
docker build -t agentspan-ui .
docker run -p 3000:80 agentspan-ui
```

Or via docker-compose from the repo root: `docker compose up ui`

## Design

The site uses a custom design system: corner-frame brackets, a live
mouse-coordinate HUD, dot-grid texture, animated chevron pill CTAs, and
a 1s per-section background cross-fade. The Network section renders all 52
channels as brand-colored avatars wearing each platform's real logo.

## Routing

- `/` — `MarketingSite` (all sections in one scroll)
- `/status` — `Dashboard` (read-only gateway overview)
