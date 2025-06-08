import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Activity,
  AlertTriangle,
  BarChart3,
  Cpu,
  Database,
  Download,
  Globe,
  Play,
  Settings,
  Square,
  TrendingUp,
  Zap,
} from "lucide-react";
import { useState } from "react";

interface BenchmarkResult {
  status: string;
  duration: number;
  requestsPerSecond: number;
  averageLatency: number;
  successRate: number;
  completedAt: string;
}

interface BenchmarkTool {
  id: string;
  name: string;
  description: string;
  category: string;
  estimatedDuration: string;
  difficulty: "Easy" | "Medium" | "Hard" | "Critical";
  command: string;
  isRunning?: boolean;
  lastRun?: string;
  icon: React.ElementType;
}

const benchmarkTools: BenchmarkTool[] = [
  {
    id: "criterion",
    name: "Criterion Benchmarks",
    description: "Statistical micro-benchmarks for precise latency measurements",
    category: "Micro-benchmarks",
    estimatedDuration: "2-3 minutes",
    difficulty: "Easy",
    command: "cargo bench --bench criterion_bench",
    icon: BarChart3,
  },
  {
    id: "k6-smoke",
    name: "K6 Smoke Test",
    description: "Quick validation test with minimal load",
    category: "Load Testing",
    estimatedDuration: "1-2 minutes",
    difficulty: "Easy",
    command: "k6 run --vus 10 --duration 30s load-test.js",
    icon: Activity,
  },
  {
    id: "goose",
    name: "Goose Load Testing",
    description: "Rust-based realistic load simulation with concurrent users",
    category: "Load Testing",
    estimatedDuration: "3-5 minutes",
    difficulty: "Medium",
    command: "cargo run --bin goose_load_test",
    icon: Cpu,
  },
  {
    id: "k6-load",
    name: "K6 Load Testing",
    description: "Standard load testing with moderate concurrent users",
    category: "Load Testing",
    estimatedDuration: "5-8 minutes",
    difficulty: "Medium",
    command: "k6 run --vus 100 --duration 300s load-test.js",
    icon: TrendingUp,
  },
  {
    id: "vegeta",
    name: "Vegeta HTTP Testing",
    description: "HTTP load testing with configurable request rates",
    category: "HTTP Testing",
    estimatedDuration: "3-5 minutes",
    difficulty: "Medium",
    command: "vegeta attack -targets=targets.txt -rate=1000/s -duration=300s",
    icon: Globe,
  },
  {
    id: "yandex-tank",
    name: "Yandex Tank",
    description: "Comprehensive load testing with detailed monitoring",
    category: "Stress Testing",
    estimatedDuration: "8-10 minutes",
    difficulty: "Hard",
    command: "yandex-tank config.yaml",
    icon: Database,
  },
  {
    id: "critical",
    name: "Critical Performance Test",
    description: "EXTREME SCALE: 100k+ RPS testing with millions of events",
    category: "Stress Testing",
    estimatedDuration: "15+ minutes",
    difficulty: "Critical",
    command: "critical-performance-test.sh",
    icon: AlertTriangle,
  },
];

export default function BenchmarksPage() {
  const [runningBenchmarks, setRunningBenchmarks] = useState<Set<string>>(new Set());
  const [selectedCategory, setSelectedCategory] = useState<string>("All");
  const [benchmarkResults, setBenchmarkResults] = useState<Record<string, BenchmarkResult>>({});

  const categories = ["All", ...Array.from(new Set(benchmarkTools.map((tool) => tool.category)))];

  const filteredTools =
    selectedCategory === "All"
      ? benchmarkTools
      : benchmarkTools.filter((tool) => tool.category === selectedCategory);

  const runBenchmark = async (tool: BenchmarkTool) => {
    if (tool.difficulty === "Critical") {
      const confirmed = confirm(
        "⚠️ WARNING: This test will generate EXTREME load (100k+ RPS)!\n\n" +
          "This could:\n" +
          "• Saturate system resources\n" +
          "• Impact other running services\n" +
          "• Take 15+ minutes to complete\n\n" +
          "Are you sure you want to proceed?"
      );
      if (!confirmed) return;
    }

    setRunningBenchmarks((prev) => new Set([...prev, tool.id]));

    try {
      // Simulate benchmark execution
      const response = await fetch("/api/benchmarks/run", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          tool: tool.id,
          command: tool.command,
        }),
      });

      // For demo purposes, simulate benchmark completion after a delay
      setTimeout(
        () => {
          setRunningBenchmarks((prev) => {
            const newSet = new Set(prev);
            newSet.delete(tool.id);
            return newSet;
          });

          // Simulate benchmark results
          setBenchmarkResults((prev) => ({
            ...prev,
            [tool.id]: {
              status: "completed",
              duration: Math.random() * 300 + 30, // 30-330 seconds
              requestsPerSecond: Math.floor(Math.random() * 5000 + 500),
              averageLatency: Math.floor(Math.random() * 100 + 10),
              successRate: 95 + Math.random() * 5,
              completedAt: new Date().toISOString(),
            },
          }));
        },
        Math.random() * 10000 + 5000
      ); // 5-15 seconds for demo
    } catch (error) {
      console.error("Failed to run benchmark:", error);
      setRunningBenchmarks((prev) => {
        const newSet = new Set(prev);
        newSet.delete(tool.id);
        return newSet;
      });
    }
  };

  const stopBenchmark = (toolId: string) => {
    setRunningBenchmarks((prev) => {
      const newSet = new Set(prev);
      newSet.delete(toolId);
      return newSet;
    });
  };

  const runAllBenchmarks = async () => {
    const confirmed = confirm(
      "Run comprehensive benchmark suite?\n\n" +
        "This will run all non-critical benchmarks sequentially.\n" +
        "Estimated total time: 20-30 minutes"
    );

    if (confirmed) {
      const nonCriticalTools = benchmarkTools.filter((tool) => tool.difficulty !== "Critical");
      for (const tool of nonCriticalTools) {
        await new Promise((resolve) => setTimeout(resolve, 2000)); // 2 second delay between tests
        runBenchmark(tool);
      }
    }
  };

  const getDifficultyColor = (difficulty: string) => {
    switch (difficulty) {
      case "Easy":
        return "success";
      case "Medium":
        return "info";
      case "Hard":
        return "warning";
      case "Critical":
        return "destructive";
      default:
        return "secondary";
    }
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">Benchmarks</h1>
          <p className="text-gray-600 mt-1">
            Performance testing suite for comprehensive system analysis
          </p>
        </div>
        <div className="flex space-x-3">
          <Button variant="outline" size="sm">
            <Download className="h-4 w-4 mr-2" />
            Export Results
          </Button>
          <Button variant="outline" size="sm">
            <Settings className="h-4 w-4 mr-2" />
            Configure
          </Button>
          <Button size="sm" onClick={runAllBenchmarks}>
            <Zap className="h-4 w-4 mr-2" />
            Run All Tests
          </Button>
        </div>
      </div>

      {/* Quick Stats */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-6">
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-medium">Available Tools</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{benchmarkTools.length}</div>
            <p className="text-xs text-blue-600">Performance testing tools</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-medium">Running</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{runningBenchmarks.size}</div>
            <p className="text-xs text-green-600">Active benchmarks</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-medium">Last Run</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">2h</div>
            <p className="text-xs text-gray-600">ago</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-medium">Success Rate</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">98.7%</div>
            <p className="text-xs text-green-600">Average across all tests</p>
          </CardContent>
        </Card>
      </div>

      {/* Category Filter */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Test Categories</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex flex-wrap gap-2">
            {categories.map((category) => (
              <Button
                key={category}
                variant={selectedCategory === category ? "default" : "outline"}
                size="sm"
                onClick={() => setSelectedCategory(category)}
              >
                {category}
                {category !== "All" &&
                  ` (${benchmarkTools.filter((t) => t.category === category).length})`}
              </Button>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Benchmark Tools Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {filteredTools.map((tool) => {
          const isRunning = runningBenchmarks.has(tool.id);
          const result = benchmarkResults[tool.id];
          const IconComponent = tool.icon;

          return (
            <Card key={tool.id} className={isRunning ? "border-blue-500 bg-blue-50" : ""}>
              <CardHeader>
                <div className="flex items-start justify-between">
                  <div className="flex items-start space-x-3">
                    <div
                      className={`p-2 rounded-lg ${
                        tool.difficulty === "Critical"
                          ? "bg-red-100"
                          : tool.difficulty === "Hard"
                            ? "bg-orange-100"
                            : tool.difficulty === "Medium"
                              ? "bg-blue-100"
                              : "bg-green-100"
                      }`}
                    >
                      <IconComponent
                        className={`h-5 w-5 ${
                          tool.difficulty === "Critical"
                            ? "text-red-600"
                            : tool.difficulty === "Hard"
                              ? "text-orange-600"
                              : tool.difficulty === "Medium"
                                ? "text-blue-600"
                                : "text-green-600"
                        }`}
                      />
                    </div>
                    <div className="flex-1">
                      <CardTitle className="text-lg">{tool.name}</CardTitle>
                      <CardDescription className="mt-1">{tool.description}</CardDescription>
                    </div>
                  </div>
                  <Badge
                    variant={
                      getDifficultyColor(tool.difficulty) as
                        | "success"
                        | "info"
                        | "warning"
                        | "destructive"
                        | "secondary"
                    }
                  >
                    {tool.difficulty}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent>
                <div className="space-y-4">
                  {/* Tool Info */}
                  <div className="grid grid-cols-2 gap-4 text-sm">
                    <div>
                      <span className="text-gray-500">Category:</span>
                      <div className="font-medium">{tool.category}</div>
                    </div>
                    <div>
                      <span className="text-gray-500">Duration:</span>
                      <div className="font-medium">{tool.estimatedDuration}</div>
                    </div>
                  </div>

                  {/* Command */}
                  <div>
                    <span className="text-gray-500 text-sm">Command:</span>
                    <div className="bg-gray-100 rounded px-3 py-2 text-sm font-mono mt-1">
                      {tool.command}
                    </div>
                  </div>

                  {/* Results */}
                  {result && (
                    <div className="bg-green-50 border border-green-200 rounded-lg p-3">
                      <div className="text-sm font-medium text-green-800 mb-2">
                        Last Run Results
                      </div>
                      <div className="grid grid-cols-2 gap-2 text-xs">
                        <div>
                          <span className="text-green-600">Duration:</span>
                          <div className="font-medium">{Math.round(result.duration)}s</div>
                        </div>
                        <div>
                          <span className="text-green-600">RPS:</span>
                          <div className="font-medium">
                            {result.requestsPerSecond.toLocaleString()}
                          </div>
                        </div>
                        <div>
                          <span className="text-green-600">Latency:</span>
                          <div className="font-medium">{result.averageLatency}ms</div>
                        </div>
                        <div>
                          <span className="text-green-600">Success:</span>
                          <div className="font-medium">{result.successRate.toFixed(1)}%</div>
                        </div>
                      </div>
                    </div>
                  )}

                  {/* Actions */}
                  <div className="flex space-x-2">
                    {isRunning ? (
                      <>
                        <Button
                          variant="destructive"
                          size="sm"
                          onClick={() => stopBenchmark(tool.id)}
                          className="flex-1"
                        >
                          <Square className="h-4 w-4 mr-2" />
                          Stop Test
                        </Button>
                        <div className="flex items-center px-3 py-2 bg-blue-100 text-blue-800 rounded text-sm">
                          <div className="animate-spin rounded-full h-3 w-3 border-b-2 border-blue-600 mr-2" />
                          Running...
                        </div>
                      </>
                    ) : (
                      <>
                        <Button
                          onClick={() => runBenchmark(tool)}
                          size="sm"
                          className="flex-1"
                          variant={tool.difficulty === "Critical" ? "destructive" : "default"}
                        >
                          <Play className="h-4 w-4 mr-2" />
                          {tool.difficulty === "Critical" ? "⚠️ Run Critical Test" : "Run Test"}
                        </Button>
                        {result && (
                          <Button variant="outline" size="sm">
                            <BarChart3 className="h-4 w-4 mr-2" />
                            View Report
                          </Button>
                        )}
                      </>
                    )}
                  </div>
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>

      {/* Running Tests Status */}
      {runningBenchmarks.size > 0 && (
        <Card className="border-blue-500">
          <CardHeader>
            <CardTitle className="flex items-center text-blue-900">
              <Activity className="h-5 w-5 mr-2 animate-pulse" />
              Active Benchmark Tests
            </CardTitle>
            <CardDescription>
              {runningBenchmarks.size} benchmark{runningBenchmarks.size !== 1 ? "s" : ""} currently
              running
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              {Array.from(runningBenchmarks).map((toolId) => {
                const tool = benchmarkTools.find((t) => t.id === toolId);
                return (
                  tool && (
                    <div
                      key={toolId}
                      className="flex items-center justify-between bg-blue-50 rounded-lg p-3"
                    >
                      <div className="flex items-center space-x-3">
                        <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-blue-600" />
                        <span className="font-medium">{tool.name}</span>
                        <Badge variant="info" className="text-xs">
                          Running
                        </Badge>
                      </div>
                      <Button variant="outline" size="sm" onClick={() => stopBenchmark(toolId)}>
                        Stop
                      </Button>
                    </div>
                  )
                );
              })}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
