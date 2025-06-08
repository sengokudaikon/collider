import {Avatar, AvatarFallback, AvatarImage} from "@/components/ui/avatar";
import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {Card, CardContent, CardDescription, CardHeader, CardTitle} from "@/components/ui/card";
import {Table, TableBody, TableCell, TableHead, TableHeader, TableRow,} from "@/components/ui/table";
import {Download, Edit, Filter, Search, Trash2, UserPlus,} from "lucide-react";
import {useState} from "react";
import useSWR from "swr";

const fetcher = (url: string) => fetch(url).then((res) => res.json());

interface User {
    id: string;
    username: string;
    events: Array<{ id: string }>;
    metrics?: {
        total_events: number;
        events_last_24h: number;
        events_last_7d: number;
        events_last_30d: number;
        most_frequent_event_type: string | null;
        event_type_counts: Array<{
            event_type: string;
            count: number;
        }>;
    };
}

export default function UsersPage() {
    const [searchTerm, setSearchTerm] = useState("");
    const [_, setShowCreateForm] = useState(false);

    const {data: users, error, mutate} = useSWR<User[]>("/api/users?include_metrics=true", fetcher);

    const filteredUsers =
        users?.filter(
            (user) =>
                user.username.toLowerCase().includes(searchTerm.toLowerCase()) ||
                user.id.toLowerCase().includes(searchTerm.toLowerCase())
        ) || [];

    const handleCreateUser = () => {
        setShowCreateForm(true);
    };

    const handleDeleteUser = async (userId: string) => {
        if (confirm("Are you sure you want to delete this user?")) {
            try {
                await fetch(`/api/users/${userId}`, {method: "DELETE"});
                mutate(); // Refresh the user list
            } catch (error) {
                console.error("Failed to delete user:", error);
            }
        }
    };

    if (error) {
        return (
            <div className="flex items-center justify-center h-96">
                <div className="text-center">
                    <p className="text-red-600 font-medium">Failed to load users</p>
                    <p className="text-gray-500 text-sm mt-1">Please check your connection and try again</p>
                </div>
            </div>
        );
    }

    return (
        <div className="space-y-6">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-3xl font-bold text-gray-900">Users</h1>
                    <p className="text-gray-600 mt-1">Manage user accounts and view user analytics</p>
                </div>
                <div className="flex space-x-3">
                    <Button variant="outline" size="sm">
                        <Download className="h-4 w-4 mr-2"/>
                        Export
                    </Button>
                    <Button size="sm" onClick={handleCreateUser}>
                        <UserPlus className="h-4 w-4 mr-2"/>
                        Add User
                    </Button>
                </div>
            </div>

            {/* Stats Cards */}
            <div className="grid grid-cols-1 md:grid-cols-4 gap-6">
                <Card>
                    <CardHeader className="pb-3">
                        <CardTitle className="text-sm font-medium">Total Users</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{users?.length || 0}</div>
                        <p className="text-xs text-green-600">+12% from last month</p>
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader className="pb-3">
                        <CardTitle className="text-sm font-medium">Active Users</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">
                            {users?.filter((u) => u.metrics && u.metrics.events_last_24h > 0).length || 0}
                        </div>
                        <p className="text-xs text-green-600">Active in last 24h</p>
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader className="pb-3">
                        <CardTitle className="text-sm font-medium">New This Week</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">24</div>
                        <p className="text-xs text-blue-600">+18% from last week</p>
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader className="pb-3">
                        <CardTitle className="text-sm font-medium">Avg Events/User</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">
                            {users?.length
                                ? Math.round(
                                    users.reduce((sum, user) => sum + (user.metrics?.total_events || 0), 0) /
                                    users.length
                                )
                                : 0}
                        </div>
                        <p className="text-xs text-gray-600">All time average</p>
                    </CardContent>
                </Card>
            </div>

            {/* Filters and Search */}
            <Card>
                <CardHeader>
                    <div className="flex items-center justify-between">
                        <div>
                            <CardTitle>User Directory</CardTitle>
                            <CardDescription>
                                A list of all users in your system including their activity metrics
                            </CardDescription>
                        </div>
                        <div className="flex items-center space-x-2">
                            <div className="relative">
                                <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-400"/>
                                <input
                                    type="text"
                                    placeholder="Search users..."
                                    className="pl-10 pr-4 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                                    value={searchTerm}
                                    onChange={(e) => setSearchTerm(e.target.value)}
                                />
                            </div>
                            <Button variant="outline" size="sm">
                                <Filter className="h-4 w-4 mr-2"/>
                                Filter
                            </Button>
                        </div>
                    </div>
                </CardHeader>
                <CardContent>
                    {!users ? (
                        <div className="flex items-center justify-center py-12">
                            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"/>
                        </div>
                    ) : (
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>User</TableHead>
                                    <TableHead>Total Events</TableHead>
                                    <TableHead>Recent Activity</TableHead>
                                    <TableHead>Top Event Type</TableHead>
                                    <TableHead>Event Types</TableHead>
                                    <TableHead className="text-right">Actions</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {filteredUsers.map((user) => (
                                    <TableRow key={user.id}>
                                        <TableCell>
                                            <div className="flex items-center space-x-3">
                                                <Avatar className="h-8 w-8">
                                                    <AvatarImage src={`https://avatar.vercel.sh/${user.username}`}/>
                                                    <AvatarFallback>{user.username.slice(0, 2).toUpperCase()}</AvatarFallback>
                                                </Avatar>
                                                <div>
                                                    <div className="font-medium text-gray-900">{user.username}</div>
                                                    <div className="text-sm text-gray-500">{user.id.slice(0, 8)}...
                                                    </div>
                                                </div>
                                            </div>
                                        </TableCell>
                                        <TableCell>
                                            <div className="font-medium">{user.metrics?.total_events || 0}</div>
                                            <div className="text-sm text-gray-500">{user.events.length} event IDs</div>
                                        </TableCell>
                                        <TableCell>
                                            <div className="space-y-1">
                                                <div className="text-sm">
                                                    <span className="text-gray-500">24h:</span>{" "}
                                                    {user.metrics?.events_last_24h || 0}
                                                </div>
                                                <div className="text-sm">
                                                    <span className="text-gray-500">7d:</span>{" "}
                                                    {user.metrics?.events_last_7d || 0}
                                                </div>
                                            </div>
                                        </TableCell>
                                        <TableCell>
                                            {user.metrics?.most_frequent_event_type ? (
                                                <Badge variant="outline">{user.metrics.most_frequent_event_type}</Badge>
                                            ) : (
                                                <span className="text-gray-400">None</span>
                                            )}
                                        </TableCell>
                                        <TableCell>
                                            <div className="text-sm">
                                                {user.metrics?.event_type_counts?.length || 0} types
                                            </div>
                                        </TableCell>
                                        <TableCell className="text-right">
                                            <div className="flex items-center justify-end space-x-2">
                                                <Button variant="ghost" size="sm">
                                                    <Edit className="h-4 w-4"/>
                                                </Button>
                                                <Button
                                                    variant="ghost"
                                                    size="sm"
                                                    onClick={() => handleDeleteUser(user.id)}
                                                    className="text-red-600 hover:text-red-700"
                                                >
                                                    <Trash2 className="h-4 w-4"/>
                                                </Button>
                                            </div>
                                        </TableCell>
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                    )}

                    {users && filteredUsers.length === 0 && (
                        <div className="text-center py-12">
                            <p className="text-gray-500">No users found matching your search.</p>
                        </div>
                    )}
                </CardContent>
            </Card>
        </div>
    );
}
