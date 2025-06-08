import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Loader } from "lucide-react";
import useSWR from "swr";

export interface UserSchema {
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

export default function UsersTable() {
  const { isLoading, data, error } = useSWR<UserSchema[]>("/api/users", (url: string) =>
    fetch(url).then((res) => res.json())
  );

  if (error) {
    return (
      <div className="text-center py-8">
        <p className="text-red-600">Failed to load users</p>
      </div>
    );
  }

  return (
    <div className="w-full">
      {isLoading && (
        <div className="flex justify-center py-8">
          <Loader className="w-6 h-6 animate-spin" />
        </div>
      )}
      {!isLoading && data && (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>User</TableHead>
              <TableHead>Events</TableHead>
              <TableHead>Activity</TableHead>
              <TableHead>Top Event Type</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {data.map((user: UserSchema) => (
              <TableRow key={user.id}>
                <TableCell>
                  <div className="flex items-center space-x-3">
                    <Avatar>
                      <AvatarImage src={`https://avatar.vercel.sh/${user.username}`} />
                      <AvatarFallback>{user.username.slice(0, 2).toUpperCase()}</AvatarFallback>
                    </Avatar>
                    <div>
                      <div className="font-medium">{user.username}</div>
                      <div className="text-sm text-gray-500">ID: {user.id.slice(0, 8)}...</div>
                    </div>
                  </div>
                </TableCell>
                <TableCell>
                  <div className="font-medium">{user.metrics?.total_events || 0}</div>
                  <div className="text-sm text-gray-500">total events</div>
                </TableCell>
                <TableCell>
                  <div className="text-sm">
                    <div>24h: {user.metrics?.events_last_24h || 0}</div>
                    <div className="text-gray-500">7d: {user.metrics?.events_last_7d || 0}</div>
                  </div>
                </TableCell>
                <TableCell>
                  {user.metrics?.most_frequent_event_type ? (
                    <Badge variant="outline" className="text-xs">
                      {user.metrics.most_frequent_event_type}
                    </Badge>
                  ) : (
                    <span className="text-gray-400 text-sm">None</span>
                  )}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
      {!isLoading && data && data.length === 0 && (
        <div className="text-center py-8">
          <p className="text-gray-500">No users found</p>
        </div>
      )}
    </div>
  );
}
