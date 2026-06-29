import { BrowserRouter, Routes, Route } from "react-router-dom";
import { MarketingSite } from "./sections/MarketingSite";
import { Dashboard } from "./dashboard/Dashboard";
import "./styles/global.css";
import "./styles/animations.css";
import "./styles/marketing.css";
import "./styles/dashboard.css";
import "./styles/responsive.css";

// Two surfaces, one design system:
//   /        → the marketing site (the whole page is the landing experience)
//   /status  → the read-only gateway dashboard
export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<MarketingSite />} />
        <Route path="/status" element={<Dashboard />} />
      </Routes>
    </BrowserRouter>
  );
}
