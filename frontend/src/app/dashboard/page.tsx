'use client';

import { useQuery } from '@tanstack/react-query';
import { apiFetch } from '@/lib/api';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Briefcase, FileText, Users, MessageSquare } from "lucide-react";
import { Skeleton } from "@/components/ui/skeleton";
import { useTranslation } from "@/lib/i18n-context";
import { cn } from "@/lib/utils";
import {
  BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Cell,
  PieChart, Pie, Legend, AreaChart, Area
} from 'recharts';

interface DashboardStats {
  total_candidates: number;
  unread_messages: number;
  active_tests: number;
  active_vacancies: number;
  candidates_by_status: Record<string, number>;
  candidates_history: [string, number][];
  attempts_status: Record<string, number>;
}

export default function DashboardPage() {
  const { t } = useTranslation();

  const { data: stats, isLoading } = useQuery({
    queryKey: ['dashboard-stats'],
    queryFn: () => apiFetch<DashboardStats>('/api/integration/dashboard/stats'),
    refetchInterval: 30000,
  });

  // 1. Candidates by Status (Bar Chart)
  const statusData = [
    { name: t('dashboard.candidates.statuses.new') || 'New', value: stats?.candidates_by_status?.['new'] || 0, color: '#3b82f6' },
    { name: t('dashboard.candidates.statuses.reviewing') || 'Reviewing', value: stats?.candidates_by_status?.['reviewing'] || 0, color: '#eab308' },
    { name: t('dashboard.candidates.statuses.accepted') || 'Accepted', value: stats?.candidates_by_status?.['accepted'] || 0, color: '#22c55e' },
    { name: t('dashboard.candidates.statuses.rejected') || 'Rejected', value: stats?.candidates_by_status?.['rejected'] || 0, color: '#ef4444' },
  ];

  // 2. Applications History (Area Chart)
  // Convert tuple array to object array
  const historyData = stats?.candidates_history?.map(([date, count]) => ({
    date: date.split('-').slice(1).join('/'), // MM/DD
    count
  })) || [];

  // 3. Test Attempts Status (Pie Chart)
  const attemptsData = [
    { name: t('dashboard.invites.statuses.pending') || 'Pending', value: stats?.attempts_status?.['pending'] || 0, color: '#94a3b8' },
    { name: t('dashboard.invites.statuses.in_progress') || 'In Progress', value: stats?.attempts_status?.['in_progress'] || 0, color: '#3b82f6' },
    { name: t('dashboard.invites.statuses.completed') || 'Completed', value: stats?.attempts_status?.['completed'] || 0, color: '#22c55e' },
    { name: t('dashboard.invites.statuses.needs_review') || 'Review Needed', value: stats?.attempts_status?.['needs_review'] || 0, color: '#eab308' },
    { name: t('dashboard.invites.statuses.timeout') || 'Timeout', value: stats?.attempts_status?.['timeout'] || 0, color: '#ef4444' },
  ].filter(d => d.value > 0);

  const StatsCard = ({ title, value, icon: Icon, colorClass, borderClass, loading, subtext }: any) => {
    return (
      <Card className={cn("premium-hover transition-shadow duration-200 hover:shadow-lg", borderClass)}>
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-3">
          <CardTitle className="text-sm font-medium text-muted-foreground">
            {title}
          </CardTitle>
          <Icon className={`h-5 w-5 ${colorClass}`} />
        </CardHeader>
        <CardContent>
          {loading ? (
            <Skeleton className="h-8 w-16 mb-1" />
          ) : (
            <div className="text-3xl font-bold">{value}</div>
          )}
          <p className="text-xs text-muted-foreground mt-1">
            {subtext}
          </p>
        </CardContent>
      </Card>
    );
  };

  return (
    <div className="space-y-8 animate-in fade-in duration-500">
      <div className="flex justify-between items-center">
        <div>
          <h2 className="text-4xl font-bold tracking-tight bg-gradient-to-r from-primary to-primary/60 bg-clip-text text-transparent">
            {t('dashboard.nav.dashboard')}
          </h2>
          <p className="text-muted-foreground mt-2 text-lg">
            {t('dashboard.stats.welcome_back') || "Overview of your recruitment pipeline."}
          </p>
        </div>
      </div>

      <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-4">
        <StatsCard
          title={t('dashboard.stats.vacancies')}
          value={stats?.active_vacancies || 0}
          icon={Briefcase}
          colorClass="text-blue-500"
          borderClass="border-l-4 border-l-blue-500"
          loading={isLoading}
          subtext={t('dashboard.stats.active_vacancies_desc') || "Open positions"}
        />

        <StatsCard
          title={t('dashboard.stats.total_candidates')}
          value={stats?.total_candidates || 0}
          icon={Users}
          colorClass="text-purple-500"
          borderClass="border-l-4 border-l-purple-500"
          loading={isLoading}
          subtext={t('dashboard.stats.total_candidates_desc') || "All time"}
        />

        <StatsCard
          title={t('dashboard.stats.unread_messages')}
          value={stats?.unread_messages || 0}
          icon={MessageSquare}
          colorClass="text-orange-500"
          borderClass="border-l-4 border-l-orange-500"
          loading={isLoading}
          subtext={t('dashboard.stats.unread_messages_desc') || "From candidates"}
        />

        <StatsCard
          title={t('dashboard.stats.active_tests')}
          value={stats?.active_tests || 0}
          icon={FileText}
          colorClass="text-green-500"
          borderClass="border-l-4 border-l-green-500"
          loading={isLoading}
          subtext={t('dashboard.stats.active_tests_desc') || "Ready to send"}
        />
      </div>

      <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
        {/* Graph 1: Applications Trend */}
        <Card className="col-span-2 premium-hover">
          <CardHeader>
            <CardTitle>{t('dashboard.stats.history') || "Applications Trend"}</CardTitle>
            <CardDescription>{t('dashboard.stats.history_desc') || "New candidates over the last 7 days"}</CardDescription>
          </CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={300}>
              <AreaChart data={historyData}>
                <defs>
                  <linearGradient id="colorCount" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#8884d8" stopOpacity={0.8} />
                    <stop offset="95%" stopColor="#8884d8" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <XAxis dataKey="date" stroke="#888888" fontSize={12} tickLine={false} axisLine={false} />
                <YAxis stroke="#888888" fontSize={12} tickLine={false} axisLine={false} tickFormatter={(value) => `${value}`} />
                <CartesianGrid strokeDasharray="3 3" vertical={false} opacity={0.2} />
                <Tooltip
                  contentStyle={{ backgroundColor: 'hsl(var(--card))', borderColor: 'hsl(var(--border))', borderRadius: '0.5rem' }}
                  itemStyle={{ color: 'hsl(var(--foreground))' }}
                />
                <Area type="monotone" dataKey="count" stroke="#8884d8" fillOpacity={1} fill="url(#colorCount)" />
              </AreaChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>

        {/* Graph 2: Test Attempts Status */}
        <Card className="col-span-1 premium-hover">
          <CardHeader>
            <CardTitle>{t('dashboard.attempts.title') || "Test Attempts"}</CardTitle>
            <CardDescription>{t('dashboard.stats.attempts_distribution') || "Distribution by status"}</CardDescription>
          </CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={300}>
              <PieChart>
                <Pie
                  data={attemptsData}
                  cx="50%"
                  cy="50%"
                  innerRadius={60}
                  outerRadius={80}
                  paddingAngle={5}
                  dataKey="value"
                >
                  {attemptsData.map((entry, index) => (
                    <Cell key={`cell-${index}`} fill={entry.color} />
                  ))}
                </Pie>
                <Tooltip
                  contentStyle={{ backgroundColor: 'hsl(var(--card))', borderColor: 'hsl(var(--border))', borderRadius: '0.5rem' }}
                  itemStyle={{ color: 'hsl(var(--foreground))' }}
                />
                <Legend verticalAlign="bottom" height={36} />
              </PieChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>

        {/* Graph 3: Candidates Pipeline - Expanded to full width if needed, or keeping it as before */}
        <Card className="col-span-3 premium-hover">
          <CardHeader>
            <CardTitle>{t('dashboard.stats.candidates_by_status') || "Candidates Overview"}</CardTitle>
            <CardDescription>{t('dashboard.stats.candidates_by_status_desc') || "Distribution of candidates across pipeline stages"}</CardDescription>
          </CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={350}>
              <BarChart data={statusData}>
                <CartesianGrid strokeDasharray="3 3" opacity={0.2} vertical={false} />
                <XAxis
                  dataKey="name"
                  stroke="#888888"
                  fontSize={12}
                  tickLine={false}
                  axisLine={false}
                />
                <YAxis
                  stroke="#888888"
                  fontSize={12}
                  tickLine={false}
                  axisLine={false}
                  tickFormatter={(value) => `${value}`}
                />
                <Tooltip
                  cursor={{ fill: 'transparent' }}
                  contentStyle={{
                    backgroundColor: 'hsl(var(--card))',
                    borderColor: 'hsl(var(--border))',
                    borderRadius: '0.5rem',
                    boxShadow: '0 4px 6px -1px rgb(0 0 0 / 0.1)'
                  }}
                  itemStyle={{ color: 'hsl(var(--foreground))' }}
                />
                <Bar dataKey="value" radius={[4, 4, 0, 0]}>
                  {statusData.map((entry, index) => (
                    <Cell key={`cell-${index}`} fill={entry.color} />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
