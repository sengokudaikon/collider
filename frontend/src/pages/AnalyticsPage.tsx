import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Activity,
  BarChart3,
  Calendar,
  Clock,
  Download,
  RefreshCw,
  Target,
  TrendingUp,
  Users,
} from "lucide-react";
import { useState } from "react";
import useSWR from "swr";

const fetcher = (url: string) => fetch(url).then((res) => res.json());

export default function AnalyticsPage() {
  const [timeRange, setTimeRange] = useState("24h");
  const [refreshing, setRefreshing] = useState(false);

  const { data: stats, mutate: mutateStats } = useSWR("/api/analytics/stats", fetcher);
  const { data: realtimeMetrics } = useSWR("/api/analytics/metrics/realtime", fetcher);
  const { data: popularEvents } = useSWR("/api/analytics/events/popular", fetcher);
  const { data: userActivity } = useSWR("/api/analytics/activity/users", fetcher);

  const handleRefreshViews = async () => {
    setRefreshing(true);
    try {
      await fetch("/api/analytics/refresh", { method: "POST" });
      mutateStats();
    } catch (error) {
      console.error("Failed to refresh materialized views:", error);
    } finally {
      setRefreshing(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">Analytics</h1>
          <p className="text-gray-600 mt-1">
            Deep insights into user behavior and application performance
          </p>
        </div>
        <div className="flex space-x-3">
          <Button variant="outline" size="sm">
            <Download className="h-4 w-4 mr-2" />
            Export Report
          </Button>
          <Button size="sm" onClick={handleRefreshViews} disabled={refreshing}>
            <RefreshCw className={`h-4 w-4 mr-2 ${refreshing ? "animate-spin" : ""}`} />
            {refreshing ? "Refreshing..." : "Refresh Data"}
          </Button>
        </div>
      </div>

      {/* Time Range Selector */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Time Range</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex space-x-2">
            {["1h", "24h", "7d", "30d"].map((range) => (
              <Button
                key={range}
                variant={timeRange === range ? "default" : "outline"}
                size="sm"
                onClick={() => setTimeRange(range)}
              >
                {range.toUpperCase()}
              </Button>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Key Metrics */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Total Events</CardTitle>
            <Activity className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{stats?.total_events?.toLocaleString() || "0"}</div>
            <p className="text-xs text-green-600">+20.1% from last period</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Unique Users</CardTitle>
            <Users className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{stats?.unique_users?.toLocaleString() || "0"}</div>
            <p className="text-xs text-blue-600">+15.2% from last period</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Events/User</CardTitle>
            <Target className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {stats?.total_events && stats?.unique_users
                ? Math.round(stats.total_events / stats.unique_users)
                : "0"}
            </div>
            <p className="text-xs text-orange-600">+5.4% from last period</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Real-time Rate</CardTitle>
            <TrendingUp className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{realtimeMetrics?.total_events || "0"}/min</div>
            <p className="text-xs text-gray-600">Current event rate</p>
          </CardContent>
        </Card>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Popular Events */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center">
              <BarChart3 className="h-5 w-5 mr-2" />
              Popular Events
            </CardTitle>
            <CardDescription>
              Most frequently triggered events in the selected time range
            </CardDescription>
          </CardHeader>
          <CardContent>
            {!popularEvents ? (
              <div className="flex items-center justify-center py-8">
                <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-blue-600" />
              </div>
            ) : popularEvents.length > 0 ? (
              <div className="space-y-4">
                {popularEvents
                  .slice(0, 5)
                  .map(
                    (
                      event: { event_type: string; unique_users: number; total_count: number },
                      index: number
                    ) => (
                      <div key={event.event_type} className="flex items-center justify-between">
                        <div className="flex items-center space-x-3">
                          <div className="flex items-center justify-center w-6 h-6 bg-blue-100 text-blue-800 rounded-full text-xs font-medium">
                            {index + 1}
                          </div>
                          <div>
                            <div className="font-medium">{event.event_type}</div>
                            <div className="text-sm text-gray-500">
                              {event.unique_users} unique users
                            </div>
                          </div>
                        </div>
                        <div className="text-right">
                          <div className="font-bold">{event.total_count?.toLocaleString()}</div>
                          <div className="text-xs text-gray-500">events</div>
                        </div>
                      </div>
                    )
                  )}
              </div>
            ) : (
              <div className="text-center py-8 text-gray-500">No event data available</div>
            )}
          </CardContent>
        </Card>

        {/* User Activity Trends */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center">
              <Users className="h-5 w-5 mr-2" />
              User Activity
            </CardTitle>
            <CardDescription>Recent user engagement and activity patterns</CardDescription>
          </CardHeader>
          <CardContent>
            {!userActivity ? (
              <div className="flex items-center justify-center py-8">
                <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-blue-600" />
              </div>
            ) : userActivity.length > 0 ? (
              <div className="space-y-4">
                {userActivity.slice(0, 5).map(
                  (activity: {
                    user_id: string;
                    event_types?: string[];
                    total_events: number;
                  }) => (
                    <div key={activity.user_id} className="flex items-center justify-between">
                      <div className="flex items-center space-x-3">
                        <div className="w-8 h-8 bg-green-100 rounded-full flex items-center justify-center">
                          <Users className="h-4 w-4 text-green-600" />
                        </div>
                        <div>
                          <div className="font-medium">User {activity.user_id.slice(0, 8)}...</div>
                          <div className="text-sm text-gray-500">
                            {activity.event_types?.join(", ") || "Various events"}
                          </div>
                        </div>
                      </div>
                      <div className="text-right">
                        <div className="font-bold">{activity.total_events}</div>
                        <div className="text-xs text-gray-500">events</div>
                      </div>
                    </div>
                  )
                )}
              </div>
            ) : (
              <div className="text-center py-8 text-gray-500">No user activity data available</div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Real-time Metrics */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center">
            <Activity className="h-5 w-5 mr-2" />
            Real-time Metrics
          </CardTitle>
          <CardDescription>Current system performance and activity metrics</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
            <div className="bg-blue-50 rounded-lg p-4">
              <div className="flex items-center justify-between">
                <div>
                  <p className="text-sm font-medium text-blue-900">Current Load</p>
                  <p className="text-2xl font-bold text-blue-900">
                    {realtimeMetrics?.total_events || 0}
                  </p>
                  <p className="text-xs text-blue-700">events/minute</p>
                </div>
                <Activity className="h-8 w-8 text-blue-600" />
              </div>
            </div>

            <div className="bg-green-50 rounded-lg p-4">
              <div className="flex items-center justify-between">
                <div>
                  <p className="text-sm font-medium text-green-900">Active Users</p>
                  <p className="text-2xl font-bold text-green-900">
                    {realtimeMetrics?.unique_users || 0}
                  </p>
                  <p className="text-xs text-green-700">in current window</p>
                </div>
                <Users className="h-8 w-8 text-green-600" />
              </div>
            </div>

            <div className="bg-purple-50 rounded-lg p-4">
              <div className="flex items-center justify-between">
                <div>
                  <p className="text-sm font-medium text-purple-900">Response Time</p>
                  <p className="text-2xl font-bold text-purple-900">23ms</p>
                  <p className="text-xs text-purple-700">avg latency</p>
                </div>
                <Clock className="h-8 w-8 text-purple-600" />
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Event Timeline Placeholder */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center">
            <Calendar className="h-5 w-5 mr-2" />
            Event Timeline
          </CardTitle>
          <CardDescription>
            Event distribution over time (chart visualization would go here)
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="h-64 bg-gray-50 rounded-lg flex items-center justify-center border-2 border-dashed border-gray-300">
            <div className="text-center">
              <BarChart3 className="h-12 w-12 text-gray-400 mx-auto mb-4" />
              <p className="text-gray-500 font-medium">Time Series Chart</p>
              <p className="text-sm text-gray-400">
                Recharts visualization would display event trends over time
              </p>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
