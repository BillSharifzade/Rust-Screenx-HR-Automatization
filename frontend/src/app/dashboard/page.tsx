'use client';

import { useQuery } from '@tanstack/react-query';
import { apiFetch } from '@/lib/api';
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Briefcase, FileText, Users, TrendingUp } from "lucide-react";
import { Skeleton } from "@/components/ui/skeleton";
import Link from 'next/link';
import { useTranslation } from "@/lib/i18n-context";
import { cn } from "@/lib/utils";

interface VacancyResponse {
  vacancies: any[];
}
interface TestsResponse {
  items: any[];
  total: number;
}
interface InvitesResponse {
  items: any[];
}

export default function DashboardPage() {
  const { t } = useTranslation();
  const { data: vacanciesData, isLoading: isLoadingVacancies } = useQuery({
    queryKey: ['external-vacancies'],
    queryFn: () => apiFetch<VacancyResponse>('/api/integration/external-vacancies'),
  });

  const { data: testsData, isLoading: isLoadingTests } = useQuery({
    queryKey: ['active-tests'],
    queryFn: () => apiFetch<TestsResponse>('/api/integration/tests?is_active=true&per_page=1'),
  });

  const { data: invitesData, isLoading: isLoadingInvites } = useQuery({
    queryKey: ['recent-invites'],
    queryFn: () => apiFetch<InvitesResponse>('/api/integration/test-invites'),
  });

  const StatsCard = ({ title, value, icon: Icon, colorClass, borderClass, loading, subtext }: any) => (
    <Card className={cn("premium-hover", borderClass)}>
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

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-4xl font-bold tracking-tight">{t('dashboard.nav.dashboard')}</h2>
        <p className="text-muted-foreground mt-2">
          {t('dashboard.tests.subtitle')}
        </p>
      </div>

      <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-4">
        <StatsCard
          title={t('dashboard.stats.vacancies')}
          value={vacanciesData?.vacancies.length || 0}
          icon={Briefcase}
          colorClass="text-blue-500"
          borderClass="border-l-4 border-l-blue-500"
          loading={isLoadingVacancies}
          subtext={t('dashboard.stats.external_source')}
        />

        <StatsCard
          title={t('dashboard.stats.active_tests')}
          value={testsData?.total || 0}
          icon={FileText}
          colorClass="text-green-500"
          borderClass="border-l-4 border-l-green-500"
          loading={isLoadingTests}
          subtext={t('dashboard.stats.available')}
        />

        <StatsCard
          title={t('dashboard.stats.recent_invites')}
          value={invitesData?.items.length || 0}
          icon={Users}
          colorClass="text-purple-500"
          borderClass="border-l-4 border-l-purple-500"
          loading={isLoadingInvites}
          subtext={t('dashboard.stats.total_sent')}
        />
      </div>

      <Card className="border-t-4 border-t-primary shadow-sm premium-hover">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <TrendingUp className="h-5 w-5" />
            {t('dashboard.quick_actions.title')}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-muted-foreground mb-4">
            {t('dashboard.quick_actions.desc')}
          </p>
          <div className="flex gap-4">
            <Link href="/dashboard/tests" className="text-primary hover:underline text-sm font-medium">
              {t('dashboard.quick_actions.go_to_tests')} &rarr;
            </Link>
            <Link href="/dashboard/vacancies" className="text-primary hover:underline text-sm font-medium">
              {t('dashboard.quick_actions.view_vacancies')} &rarr;
            </Link>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
