import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { HomePage } from "./pages/HomePage";
import { ReviewPage } from "./pages/ReviewPage";
import { SettingsPage } from "./pages/SettingsPage";
import { ScanPage } from "./pages/ScanPage";

export default function App() {
  return (
    <BrowserRouter>
      <div className="h-full min-h-screen bg-background text-foreground">
        <Routes>
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="/" element={<HomePage />} />
          <Route path="/scan/:sessionId" element={<ScanPage />} />
          <Route path="/review/:sessionId" element={<ReviewPage />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </div>
    </BrowserRouter>
  );
}
