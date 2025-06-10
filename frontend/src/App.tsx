import {BrowserRouter as Router, Route, Routes} from "react-router-dom";
import Header from "./components/layout/Header";
import Sidebar from "./components/layout/Sidebar";
import AnalyticsPage from "./pages/AnalyticsPage";
import BenchmarksPage from "./pages/BenchmarksPage";
import Dashboard from "./pages/Dashboard";
import EventsPage from "./pages/EventsPage";
import UsersPage from "./pages/UsersPage";

function App() {
  return (
    <Router>
      <div className="flex h-screen bg-gray-50">
        <Sidebar />
        <div className="flex-1 flex flex-col overflow-hidden">
          <Header />
          <main className="flex-1 overflow-y-auto p-6">
            <Routes>
              <Route path="/" element={<Dashboard />} />
              <Route path="/users" element={<UsersPage />} />
              <Route path="/events" element={<EventsPage />} />
              <Route path="/analytics" element={<AnalyticsPage />} />
              <Route path="/benchmarks" element={<BenchmarksPage />} />
            </Routes>
          </main>
        </div>
      </div>
    </Router>
  );
}

export default App;
