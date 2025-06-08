import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {Card, CardContent, CardDescription, CardHeader, CardTitle} from "@/components/ui/card";
import {Activity, BarChart3, Calendar, Clock, RefreshCw, TrendingUp, Users, Zap,} from "lucide-react";
import useSWR from "swr";

const fetcher = (url: string) => fetch(url).then((res) => res.json());

export default function Dashboard() {
    const {data: stats, error: statsError} = useSWR("/api/analytics/stats", fetcher);
    const {data: realtimeMetrics} = useSWR("/api/analytics/metrics/realtime", fetcher);

    return (
        <div className="space-y-6">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-3xl font-bold text-gray-900">Dashboard</h1>
                    <p className="text-gray-600 mt-1">
                        Monitor your Collider application performance and metrics
                    </p>
                </div>
                <div className="flex space-x-3">
                    <Button variant="outline" size="sm">
                        <RefreshCw className="h-4 w-4 mr-2"/>
                        Refresh Data
                    </Button>
                    <Button size="sm">
                        <Zap className="h-4 w-4 mr-2"/>
                        Run Benchmark
                    </Button>
                </div>
            </div>

            {/* Quick Stats */}
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Total Events</CardTitle>
                        <Calendar className="h-4 w-4 text-muted-foreground"/>
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{stats?.total_events?.toLocaleString() || "0"}</div>
                        <p className="text-xs text-muted-foreground">+12% from last month</p>
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Active Users</CardTitle>
                        <Users className="h-4 w-4 text-muted-foreground"/>
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{stats?.unique_users?.toLocaleString() || "0"}</div>
                        <p className="text-xs text-muted-foreground">+5% from last month</p>
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Response Time</CardTitle>
                        <Clock className="h-4 w-4 text-muted-foreground"/>
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">23ms</div>
                        <p className="text-xs text-muted-foreground">Average p95 latency</p>
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Throughput</CardTitle>
                        <TrendingUp className="h-4 w-4 text-muted-foreground"/>
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">2.4k</div>
                        <p className="text-xs text-muted-foreground">Requests per minute</p>
                    </CardContent>
                </Card>
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                {/* System Health */}
                <Card>
                    <CardHeader>
                        <CardTitle className="flex items-center">
                            <Activity className="h-5 w-5 mr-2"/>
                            System Health
                        </CardTitle>
                        <CardDescription>Current status of all system components</CardDescription>
                    </CardHeader>
                    <CardContent className="space-y-4">
                        <div className="flex items-center justify-between">
                            <span className="text-sm font-medium">API Server</span>
                            <Badge variant="success">Healthy</Badge>
                        </div>
                        <div className="flex items-center justify-between">
                            <span className="text-sm font-medium">Database</span>
                            <Badge variant="success">Healthy</Badge>
                        </div>
                        <div className="flex items-center justify-between">
                            <span className="text-sm font-medium">Redis Cache</span>
                            <Badge variant="success">Healthy</Badge>
                        </div>
                        <div className="flex items-center justify-between">
                            <span className="text-sm font-medium">Background Jobs</span>
                            <Badge variant="warning">Degraded</Badge>
                        </div>
                    </CardContent>
                </Card>

                {/* Top Pages */}
                <Card>
                    <CardHeader>
                        <CardTitle className="flex items-center">
                            <BarChart3 className="h-5 w-5 mr-2"/>
                            Top Event Types
                        </CardTitle>
                        <CardDescription>Most popular events in the last 24 hours</CardDescription>
                    </CardHeader>
                    <CardContent>
                        <div className="space-y-3">
                            {stats?.top_pages &&
                                Object.entries(stats.top_pages).map(([page, count]) => (
                                    <div key={page} className="flex items-center justify-between">
                                        <span className="text-sm font-medium">{page}</span>
                                        <div className="flex items-center space-x-2">
                                            <div className="w-20 bg-gray-200 rounded-full h-2">
                                                <div
                                                    className="bg-blue-600 h-2 rounded-full"
                                                    style={{
                                                        width: `${Math.min(((count as number) / Math.max(...Object.values(stats.top_pages))) * 100, 100)}%`,
                                                    }}
                                                />
                                            </div>
                                            <span className="text-sm text-gray-600">
                        {(count as number).toLocaleString()}
                      </span>
                                        </div>
                                    </div>
                                ))}
                        </div>
                    </CardContent>
                </Card>
            </div>

            {/* Quick Actions */}
            <Card>
                <CardHeader>
                    <CardTitle className="flex items-center">
                        <Zap className="h-5 w-5 mr-2"/>
                        Quick Actions
                    </CardTitle>
                    <CardDescription>Common administrative tasks and tools</CardDescription>
                </CardHeader>
                <CardContent>
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                        <Button variant="outline" className="h-auto flex-col py-4">
                            <Users className="h-6 w-6 mb-2"/>
                            <span className="font-medium">Manage Users</span>
                            <span className="text-xs text-gray-500">Create, edit, delete users</span>
                        </Button>
                        <Button variant="outline" className="h-auto flex-col py-4">
                            <Calendar className="h-6 w-6 mb-2"/>
                            <span className="font-medium">View Events</span>
                            <span className="text-xs text-gray-500">Browse event history</span>
                        </Button>
                        <Button variant="outline" className="h-auto flex-col py-4">
                            <BarChart3 className="h-6 w-6 mb-2"/>
                            <span className="font-medium">Analytics</span>
                            <span className="text-xs text-gray-500">Detailed metrics</span>
                        </Button>
                        <Button variant="outline" className="h-auto flex-col py-4">
                            <Zap className="h-6 w-6 mb-2"/>
                            <span className="font-medium">Benchmarks</span>
                            <span className="text-xs text-gray-500">Performance testing</span>
                        </Button>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
