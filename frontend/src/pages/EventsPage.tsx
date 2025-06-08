import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Calendar, Clock, Download, Eye, Filter, Plus, Search, Trash2, User } from "lucide-react";
import { useState } from "react";
import useSWR from "swr";

const fetcher = (url: string) => fetch(url).then((res) => res.json());

interface Event {
  id: string;
  user_id: string;
  event_type_id: number;
  timestamp: string;
  metadata?: Record<string, unknown>;
}

interface EventsPageProps {
  events?: Event[];
  isLoading: boolean;
  onDeleteEvent: (eventId: string) => Promise<void>;
  onCreateSample: () => Promise<void>;
}

function EventsHeader({ onCreateSample }: { onCreateSample: () => Promise<void> }) {
  return (
    <div className="flex items-center justify-between">
      <div>
        <h1 className="text-3xl font-bold text-gray-900">Events</h1>
        <p className="text-gray-600 mt-1">Track and manage application events in real-time</p>
      </div>
      <div className="flex space-x-3">
        <Button variant="outline" size="sm">
          <Download className="h-4 w-4 mr-2" />
          Export CSV
        </Button>
        <Button size="sm" onClick={onCreateSample}>
          <Plus className="h-4 w-4 mr-2" />
          Create Sample Event
        </Button>
      </div>
    </div>
  );
}

function EventsStats({ events, eventTypes }: { events?: Event[]; eventTypes: string[] }) {
  return (
    <div className="grid grid-cols-1 md:grid-cols-4 gap-6">
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm font-medium">Total Events</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="text-2xl font-bold">{events?.length || 0}</div>
          <p className="text-xs text-green-600">Last 50 events shown</p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm font-medium">Event Types</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="text-2xl font-bold">{eventTypes.length}</div>
          <p className="text-xs text-blue-600">Unique event types</p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm font-medium">Today's Events</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="text-2xl font-bold">
            {events?.filter(
              (e) => new Date(e.timestamp).toDateString() === new Date().toDateString()
            ).length || 0}
          </div>
          <p className="text-xs text-gray-600">Events created today</p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm font-medium">Avg/Hour</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="text-2xl font-bold">127</div>
          <p className="text-xs text-gray-600">Events per hour</p>
        </CardContent>
      </Card>
    </div>
  );
}

function EventTypeFilter({
  events,
  eventTypes,
  selectedEventType,
  onTypeChange,
}: {
  events?: Event[];
  eventTypes: string[];
  selectedEventType: string;
  onTypeChange: (type: string) => void;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-lg">Event Types</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex flex-wrap gap-2">
          <Button
            variant={selectedEventType === "" ? "default" : "outline"}
            size="sm"
            onClick={() => onTypeChange("")}
          >
            All Types ({events?.length || 0})
          </Button>
          {eventTypes.map((typeId) => (
            <Button
              key={typeId}
              variant={selectedEventType === typeId ? "default" : "outline"}
              size="sm"
              onClick={() => onTypeChange(typeId)}
            >
              Type {typeId} (
              {events?.filter((e) => e.event_type_id.toString() === typeId).length || 0})
            </Button>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

function EventsTable({
  events,
  filteredEvents,
  searchTerm,
  onSearchChange,
  onDeleteEvent,
  onCreateSample,
}: {
  events?: Event[];
  filteredEvents: Event[];
  searchTerm: string;
  onSearchChange: (term: string) => void;
  onDeleteEvent: (eventId: string) => Promise<void>;
  onCreateSample: () => Promise<void>;
}) {
  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between">
          <div>
            <CardTitle>Event Stream</CardTitle>
            <CardDescription>
              Real-time view of application events and user interactions
            </CardDescription>
          </div>
          <div className="flex items-center space-x-2">
            <div className="relative">
              <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-400" />
              <input
                type="text"
                placeholder="Search events..."
                className="pl-10 pr-4 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                value={searchTerm}
                onChange={(e) => onSearchChange(e.target.value)}
              />
            </div>
            <Button variant="outline" size="sm">
              <Filter className="h-4 w-4 mr-2" />
              Advanced Filter
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        {!events ? (
          <div className="flex items-center justify-center py-12">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" />
          </div>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Event Type ID</TableHead>
                <TableHead>User ID</TableHead>
                <TableHead>Timestamp</TableHead>
                <TableHead>Metadata</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filteredEvents.map((event) => (
                <TableRow key={event.id}>
                  <TableCell>
                    <Badge variant="outline" className="font-mono">
                      Type {event.event_type_id}
                    </Badge>
                  </TableCell>
                  <TableCell>
                    <div className="flex items-center space-x-2">
                      <User className="h-4 w-4 text-gray-400" />
                      <span className="font-mono text-sm">{event.user_id.slice(0, 8)}...</span>
                    </div>
                  </TableCell>
                  <TableCell>
                    <div className="flex items-center space-x-2">
                      <Clock className="h-4 w-4 text-gray-400" />
                      <div>
                        <div className="text-sm font-medium">
                          {new Date(event.timestamp).toLocaleTimeString()}
                        </div>
                        <div className="text-xs text-gray-500">
                          {new Date(event.timestamp).toLocaleDateString()}
                        </div>
                      </div>
                    </div>
                  </TableCell>
                  <TableCell>
                    {event.metadata ? (
                      <div className="max-w-xs">
                        <pre className="text-xs text-gray-600 truncate">
                          {JSON.stringify(event.metadata, null, 0)}
                        </pre>
                      </div>
                    ) : (
                      <span className="text-gray-400">No metadata</span>
                    )}
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex items-center justify-end space-x-2">
                      <Button variant="ghost" size="sm">
                        <Eye className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => onDeleteEvent(event.id)}
                        className="text-red-600 hover:text-red-700"
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}

        {events && filteredEvents.length === 0 && (
          <div className="text-center py-12">
            <Calendar className="h-12 w-12 text-gray-400 mx-auto mb-4" />
            <p className="text-gray-500">No events found matching your criteria.</p>
            <Button className="mt-4" onClick={onCreateSample}>
              Create Sample Event
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export default function EventsPage() {
  const [searchTerm, setSearchTerm] = useState("");
  const [selectedEventType, setSelectedEventType] = useState<string>("");
  const [page, setPage] = useState(1);

  const {
    data: events,
    error,
    mutate,
  } = useSWR<Event[]>(`/api/events?limit=50&page=${page}`, fetcher);

  const eventTypes = Array.from(new Set(events?.map((e) => e.event_type_id.toString()) || []));

  const filteredEvents =
    events?.filter((event) => {
      const matchesSearch =
        event.event_type_id.toString().includes(searchTerm) ||
        event.user_id.toLowerCase().includes(searchTerm.toLowerCase());

      const matchesType =
        !selectedEventType || event.event_type_id.toString() === selectedEventType;

      return matchesSearch && matchesType;
    }) || [];

  const handleDeleteEvent = async (eventId: string) => {
    if (confirm("Are you sure you want to delete this event?")) {
      try {
        await fetch(`/api/events/${eventId}`, { method: "DELETE" });
        mutate();
      } catch (error) {
        console.error("Failed to delete event:", error);
      }
    }
  };

  const createSampleEvent = async () => {
    try {
      const sampleEvent = {
        user_id: crypto.randomUUID(),
        event_type_id: 1, // assuming event type 1 exists
        metadata: {
          action: "button_click",
          page: "/dashboard",
          element: "sample_button",
        },
      };

      await fetch("/api/events", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(sampleEvent),
      });

      mutate();
    } catch (error) {
      console.error("Failed to create sample event:", error);
    }
  };

  if (error) {
    return (
      <div className="flex items-center justify-center h-96">
        <div className="text-center">
          <p className="text-red-600 font-medium">Failed to load events</p>
          <p className="text-gray-500 text-sm mt-1">Please check your connection and try again</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <EventsHeader onCreateSample={createSampleEvent} />
      <EventsStats events={events} eventTypes={eventTypes} />
      <EventTypeFilter
        events={events}
        eventTypes={eventTypes}
        selectedEventType={selectedEventType}
        onTypeChange={setSelectedEventType}
      />
      <EventsTable
        events={events}
        filteredEvents={filteredEvents}
        searchTerm={searchTerm}
        onSearchChange={setSearchTerm}
        onDeleteEvent={handleDeleteEvent}
        onCreateSample={createSampleEvent}
      />

      {/* Pagination */}
      {events && events.length >= 50 && (
        <div className="flex items-center justify-center space-x-2">
          <Button
            variant="outline"
            size="sm"
            disabled={page === 1}
            onClick={() => setPage(page - 1)}
          >
            Previous
          </Button>
          <span className="text-sm text-gray-600">Page {page}</span>
          <Button variant="outline" size="sm" onClick={() => setPage(page + 1)}>
            Next
          </Button>
        </div>
      )}
    </div>
  );
}
