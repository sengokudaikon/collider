import {cn} from "@/lib/utils";
import {Activity, BarChart3, Calendar, Home, Users, Zap} from "lucide-react";
import {Link, useLocation} from "react-router-dom";

const navigation = [
  { name: "Dashboard", href: "/", icon: Home },
  { name: "Users", href: "/users", icon: Users },
  { name: "Events", href: "/events", icon: Calendar },
  { name: "Analytics", href: "/analytics", icon: BarChart3 },
  { name: "Benchmarks", href: "/benchmarks", icon: Zap },
];

export default function Sidebar() {
  const location = useLocation();

  return (
    <div className="flex h-full w-64 flex-col bg-white border-r border-gray-200">
      {/* Logo */}
      <div className="flex h-16 items-center px-6 border-b border-gray-200">
        <div className="flex items-center space-x-2">
          <Activity className="h-8 w-8 text-blue-600" />
          <div className="flex flex-col">
            <span className="text-xl font-bold text-gray-900">Collider</span>
            <span className="text-xs text-gray-500">Performance Dashboard</span>
          </div>
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 px-4 py-6 space-y-1">
        {navigation.map((item) => {
          const isActive = location.pathname === item.href;
          return (
            <Link
              key={item.name}
              to={item.href}
              className={cn(
                "group flex items-center px-3 py-2 text-sm font-medium rounded-lg transition-colors",
                isActive
                  ? "bg-blue-50 text-blue-700 border-r-2 border-blue-700"
                  : "text-gray-700 hover:bg-gray-50 hover:text-gray-900"
              )}
            >
              <item.icon
                className={cn(
                  "mr-3 h-5 w-5 transition-colors",
                  isActive ? "text-blue-600" : "text-gray-400 group-hover:text-gray-600"
                )}
              />
              {item.name}
            </Link>
          );
        })}
      </nav>

      {/* System Status */}
      <div className="border-t border-gray-200 p-4">
        <div className="bg-green-50 border border-green-200 rounded-lg p-3">
          <div className="flex items-center">
            <div className="h-2 w-2 bg-green-500 rounded-full mr-2" />
            <span className="text-sm font-medium text-green-800">System Online</span>
          </div>
          <p className="text-xs text-green-700 mt-1">All services operational</p>
        </div>
      </div>
    </div>
  );
}
