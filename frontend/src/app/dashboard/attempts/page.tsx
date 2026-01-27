'use client';

import { useState } from 'react';

import { useQuery, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from '@/lib/api';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Clock, CheckCircle, XCircle, User, FileText, ExternalLink, Loader2, AlertCircle } from 'lucide-react';
import { format } from 'date-fns';
import { enUS, ru as ruLocale } from 'date-fns/locale';
import { cn } from '@/lib/utils';
import { useTranslation } from '@/lib/i18n-context';
import Link from 'next/link';
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";

interface TestAttempt {
    id: string;
    test_id: string;
    candidate_name: string;
    candidate_email: string;
    status: string;
    score?: number;
    max_score?: number;
    percentage?: number;
    started_at?: string;
    submitted_at?: string;
    expires_at: string;
    passed?: boolean;
}

interface TestAttemptsResponse {
    items: TestAttempt[];
    total: number;
    page: number;
    limit: number;
}

export default function TestAttemptsPage() {
    const { t, language } = useTranslation();
    const dateLocale = language === 'ru' ? ruLocale : enUS;
    const [statusFilter, setStatusFilter] = useState<string>("all");

    const { data, isLoading, error } = useQuery({
        queryKey: ['test-attempts', statusFilter],
        queryFn: () => {
            const url = new URL('/api/integration/test-attempts', window.location.origin);
            if (statusFilter !== 'all') {
                url.searchParams.append('status', statusFilter);
            }
            return apiFetch<TestAttemptsResponse>(url.toString());
        },
    });



    if (isLoading) {
        return (
            <div className="flex items-center justify-center h-64">
                <div className="text-muted-foreground">{t('dashboard.attempts.loading')}</div>
            </div>
        );
    }

    if (error) {
        return (
            <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-4">
                <p className="text-destructive font-medium">{t('dashboard.attempts.error')}</p>
                <p className="text-sm text-destructive/80 mt-1">{error.message}</p>
            </div>
        );
    }

    const getStatusBadge = (attempt: TestAttempt) => {
        const status = attempt.status;
        const variants: Record<string, { variant: any; icon: any; label: string }> = {
            completed: { variant: 'default', icon: CheckCircle, label: t('dashboard.attempts.statuses.completed') },
            in_progress: { variant: 'secondary', icon: Loader2, label: t('dashboard.attempts.statuses.in_progress') },
            expired: { variant: 'outline', icon: XCircle, label: t('dashboard.attempts.statuses.expired') },
            timeout: { variant: 'outline', icon: Clock, label: t('dashboard.attempts.statuses.timeout') },
            escaped: { variant: 'outline', icon: XCircle, label: t('dashboard.attempts.statuses.escaped') },
            needs_review: { variant: 'secondary', icon: AlertCircle, label: t('dashboard.attempts.statuses.needs_review') },
        };

        if (status === 'completed' && attempt.passed !== undefined) {
            if (attempt.passed) {
                return (
                    <Badge variant="default" className="gap-1 bg-green-600 hover:bg-green-700">
                        <CheckCircle className="h-3 w-3" />
                        {t('dashboard.attempts.labels.passed')}
                    </Badge>
                );
            } else {
                return (
                    <Badge variant="destructive" className="gap-1">
                        <XCircle className="h-3 w-3" />
                        {t('dashboard.attempts.labels.failed')}
                    </Badge>
                );
            }
        }

        const config = variants[status] || { ...variants.in_progress, label: status };
        const Icon = config.icon;

        if (status === 'needs_review') {
            return (
                <Badge className="gap-1 bg-amber-500 hover:bg-amber-600 text-white border-none">
                    <Icon className="h-3 w-3" />
                    {config.label}
                </Badge>
            );
        }

        return (
            <Badge variant={config.variant} className="gap-1">
                <Icon className="h-3 w-3" />
                {config.label}
            </Badge>
        );
    };

    return (
        <div className="space-y-6">
            <div className="flex items-start justify-between">
                <div className="space-y-1">
                    <h3 className="text-2xl font-bold tracking-tight">{t('dashboard.attempts.title')}</h3>
                    <p className="text-muted-foreground">
                        {t('dashboard.attempts.subtitle')}
                    </p>
                </div>
                <div className="flex items-center gap-3">
                    <Select value={statusFilter} onValueChange={setStatusFilter}>
                        <SelectTrigger className="w-[180px]">
                            <SelectValue placeholder={t('common.filter') || "Filter"} />
                        </SelectTrigger>
                        <SelectContent>
                            <SelectItem value="all">{t('common.all_statuses') || "All Statuses"}</SelectItem>
                            <SelectItem value="pending">{t('dashboard.attempts.statuses.pending')}</SelectItem>
                            <SelectItem value="in_progress">{t('dashboard.attempts.statuses.in_progress')}</SelectItem>
                            <SelectItem value="completed">{t('dashboard.attempts.statuses.completed')}</SelectItem>
                            <SelectItem value="needs_review">{t('dashboard.attempts.statuses.needs_review')}</SelectItem>
                            <SelectItem value="expired">{t('dashboard.attempts.statuses.expired')}</SelectItem>
                            <SelectItem value="timeout">{t('dashboard.attempts.statuses.timeout')}</SelectItem>
                            <SelectItem value="escaped">{t('dashboard.attempts.statuses.escaped')}</SelectItem>
                        </SelectContent>
                    </Select>
                    <Badge variant="secondary" className="text-xs h-9 px-3 flex items-center justify-center">
                        {data?.total || 0} {t('dashboard.attempts.total')}
                    </Badge>
                </div>
            </div>

            <div className="grid gap-4">
                {data?.items.map((attempt) => (
                    <Card key={attempt.id} className={cn("premium-hover", attempt.status === 'needs_review' && "border-amber-500/50 bg-amber-500/5 dark:bg-amber-500/10")}>
                        <CardHeader className="flex flex-row items-start justify-between space-y-0 pb-3">
                            <div className="space-y-2 flex-1">
                                <div className="flex items-center gap-3">
                                    <User className="h-5 w-5 text-muted-foreground" />
                                    <div>
                                        <CardTitle className="text-lg">{attempt.candidate_name}</CardTitle>
                                        <p className="text-sm text-muted-foreground">{attempt.candidate_email}</p>
                                    </div>
                                </div>
                            </div>
                            <div className="flex items-center gap-2">
                                {getStatusBadge(attempt)}
                                <Button variant="ghost" size="sm" className="gap-2" asChild>
                                    <Link href={`/dashboard/attempts/${attempt.id}`}>
                                        <ExternalLink className="h-4 w-4" />
                                        {t('common.view') || 'View'}
                                    </Link>
                                </Button>
                            </div>
                        </CardHeader>
                        <CardContent>
                            <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
                                {attempt.score !== undefined && (
                                    <div className="space-y-1">
                                        <p className="text-muted-foreground text-xs">{t('dashboard.attempts.labels.score')}</p>
                                        <p className="font-semibold text-lg">
                                            {attempt.score} / {attempt.max_score}
                                            <span className="text-sm text-muted-foreground ml-2">
                                                ({attempt.percentage}%)
                                            </span>
                                        </p>
                                    </div>
                                )}
                                {attempt.started_at && (
                                    <div className="space-y-1">
                                        <p className="text-muted-foreground text-xs">{t('dashboard.attempts.labels.started_at')}</p>
                                        <p className="font-medium">
                                            {format(new Date(attempt.started_at), 'PPp', { locale: dateLocale })}
                                        </p>
                                    </div>
                                )}
                                {attempt.submitted_at && (
                                    <div className="space-y-1">
                                        <p className="text-muted-foreground text-xs">{t('dashboard.attempts.labels.submitted_at')}</p>
                                        <p className="font-medium">
                                            {format(new Date(attempt.submitted_at), 'PPp', { locale: dateLocale })}
                                        </p>
                                    </div>
                                )}
                                <div className="space-y-1">
                                    <p className="text-muted-foreground text-xs">{t('dashboard.attempts.labels.expires_at')}</p>
                                    <p className="font-medium">
                                        {format(new Date(attempt.expires_at), 'PPp', { locale: dateLocale })}
                                    </p>
                                </div>
                            </div>
                        </CardContent>
                    </Card>
                ))}

                {data?.items.length === 0 && (
                    <Card className="border-dashed">
                        <CardContent className="flex flex-col items-center justify-center h-48 text-center">
                            <FileText className="h-12 w-12 text-muted-foreground/50 mb-4" />
                            <p className="text-muted-foreground font-medium">{t('dashboard.attempts.empty')}</p>
                            <p className="text-sm text-muted-foreground/70 mt-1">
                                {t('dashboard.attempts.empty_desc')}
                            </p>
                        </CardContent>
                    </Card>
                )}
            </div>
        </div>
    );
}
