'use client';

import { useParams, useRouter } from 'next/navigation';
import { useQuery } from '@tanstack/react-query';
import { apiFetch } from '@/lib/api';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
    ChevronLeft,
    User,
    Mail,
    Phone,
    Send,
    Clock,
    CheckCircle,
    XCircle,
    FileText,
    AlertCircle,
    Target,
    BarChart3
} from 'lucide-react';
import { useTranslation } from '@/lib/i18n-context';
import { format } from 'date-fns';
import { enUS, ru as ruLocale } from 'date-fns/locale';
import { cn } from "@/lib/utils";

interface GradedAnswer {
    question_id: number;
    question_text: string;
    type: string;
    candidate_answer: any;
    correct_answer?: any;
    is_correct: boolean;
    points_earned: number;
    max_points: number;
    explanation?: string;
}

interface AttemptDetails {
    id: string;
    test: {
        id: string;
        title: string;
    };
    candidate: {
        external_id?: string;
        name: string;
        email: string;
        telegram_id?: number;
        phone?: string;
    };
    status: string;
    score?: number;
    max_score?: number;
    percentage?: number;
    passed?: boolean;
    started_at?: string;
    completed_at?: string;
    time_spent_seconds?: number;
    graded_answers?: GradedAnswer[];
    metadata?: any;
}

export default function AttemptDetailsPage() {
    const { t, language } = useTranslation();
    const params = useParams();
    const router = useRouter();
    const id = params.id as string;
    const dateLocale = language === 'ru' ? ruLocale : enUS;

    const { data: attempt, isLoading, error } = useQuery({
        queryKey: ['test-attempt', id],
        queryFn: () => apiFetch<AttemptDetails>(`/api/integration/test-attempts/${id}`),
    });

    if (isLoading) {
        return (
            <div className="flex items-center justify-center min-h-[400px]">
                <div className="text-muted-foreground animate-pulse">{t('common.loading')}</div>
            </div>
        );
    }

    if (error || !attempt) {
        return (
            <div className="space-y-4">
                <Button variant="ghost" onClick={() => router.back()} className="gap-2">
                    <ChevronLeft className="h-4 w-4" />
                    {t('common.back')}
                </Button>
                <Card className="border-destructive/50 bg-destructive/10">
                    <CardHeader>
                        <CardTitle className="text-destructive flex items-center gap-2">
                            <AlertCircle className="h-5 w-5" />
                            {t('dashboard.attempts.error')}
                        </CardTitle>
                    </CardHeader>
                    <CardContent>
                        <p className="text-destructive/80">{(error as any)?.message || "Failed to load attempt details"}</p>
                    </CardContent>
                </Card>
            </div>
        );
    }

    const getStatusBadge = (status: string) => {
        const variants: Record<string, { variant: any; icon: any; label: string }> = {
            completed: { variant: 'default', icon: CheckCircle, label: t('dashboard.attempts.statuses.completed') },
            active: { variant: 'secondary', icon: Clock, label: t('dashboard.attempts.statuses.active') },
            expired: { variant: 'outline', icon: XCircle, label: t('dashboard.attempts.statuses.expired') },
            pending: { variant: 'secondary', icon: Clock, label: t('dashboard.invites.statuses.pending') },
        };

        const config = variants[status] || { variant: 'outline', icon: AlertCircle, label: status };
        const Icon = config.icon;

        return (
            <Badge variant={config.variant} className="gap-1">
                <Icon className="h-3 w-3" />
                {config.label}
            </Badge>
        );
    };

    return (
        <div className="space-y-6 max-w-5xl mx-auto">
            <div className="flex items-center justify-between">
                <Button variant="ghost" onClick={() => router.back()} className="gap-2">
                    <ChevronLeft className="h-4 w-4" />
                    {t('common.back')}
                </Button>
                <div className="flex items-center gap-3">
                    {getStatusBadge(attempt.status)}
                    {attempt.passed !== undefined && (
                        <Badge variant={attempt.passed ? "default" : "destructive"} className={attempt.passed ? "bg-green-600" : ""}>
                            {attempt.passed ? "PASSED" : "FAILED"}
                        </Badge>
                    )}
                </div>
            </div>

            <div className="grid gap-6 md:grid-cols-3">
                {/* Candidate Info */}
                <Card className="md:col-span-1 premium-hover border-primary/10">
                    <CardHeader>
                        <CardTitle className="text-lg flex items-center gap-2">
                            <User className="h-5 w-5 text-primary" />
                            {t('dashboard.invites.create.candidate')}
                        </CardTitle>
                    </CardHeader>
                    <CardContent className="space-y-4">
                        <div className="space-y-1">
                            <p className="text-sm font-semibold">{attempt.candidate.name}</p>
                            <div className="flex items-center gap-2 text-sm text-muted-foreground">
                                <Mail className="h-4 w-4 shrink-0" />
                                <span className="truncate">{attempt.candidate.email}</span>
                            </div>
                        </div>

                        {attempt.candidate.phone && (
                            <div className="flex items-center gap-2 text-sm text-muted-foreground">
                                <Phone className="h-4 w-4 shrink-0" />
                                <span>{attempt.candidate.phone}</span>
                            </div>
                        )}

                        {attempt.candidate.telegram_id && (
                            <div className="flex items-center gap-2 text-sm text-muted-foreground">
                                <Send className="h-4 w-4 shrink-0 text-blue-500" />
                                <span>ID: {attempt.candidate.telegram_id}</span>
                            </div>
                        )}

                        {attempt.candidate.external_id && (
                            <div className="pt-2 border-t mt-2">
                                <p className="text-[10px] text-muted-foreground uppercase tracking-wider">External ID</p>
                                <p className="text-xs font-mono">{attempt.candidate.external_id}</p>
                            </div>
                        )}
                    </CardContent>
                </Card>

                {/* Test Results Summary */}
                <Card className="md:col-span-2 premium-hover border-primary/10">
                    <CardHeader>
                        <CardTitle className="text-lg flex items-center gap-2">
                            <BarChart3 className="h-5 w-5 text-primary" />
                            {t('dashboard.attempts.card.result')} Details
                        </CardTitle>
                        <CardDescription>{attempt.test.title}</CardDescription>
                    </CardHeader>
                    <CardContent>
                        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
                            <div className="space-y-1">
                                <p className="text-xs text-muted-foreground uppercase">{t('dashboard.attempts.labels.score')}</p>
                                <p className="text-2xl font-bold">
                                    {attempt.score ?? 0} <span className="text-sm font-normal text-muted-foreground">/ {attempt.max_score}</span>
                                </p>
                            </div>
                            <div className="space-y-1">
                                <p className="text-xs text-muted-foreground uppercase">{t('dashboard.attempts.labels.percentage')}</p>
                                <p className="text-2xl font-bold text-primary">
                                    {attempt.percentage ?? 0}%
                                </p>
                            </div>
                            <div className="space-y-1">
                                <p className="text-xs text-muted-foreground uppercase">{t('dashboard.attempts.labels.started_at')}</p>
                                <p className="text-sm font-medium">
                                    {attempt.started_at ? format(new Date(attempt.started_at), 'Pp', { locale: dateLocale }) : '—'}
                                </p>
                            </div>
                            <div className="space-y-1">
                                <p className="text-xs text-muted-foreground uppercase">{t('dashboard.attempts.labels.time_spent')}</p>
                                <p className="text-sm font-medium">
                                    {(() => {
                                        const totalSeconds = attempt.time_spent_seconds || (attempt.started_at && attempt.completed_at
                                            ? Math.round((new Date(attempt.completed_at).getTime() - new Date(attempt.started_at).getTime()) / 1000)
                                            : 0);
                                        if (!totalSeconds) return '—';
                                        const h = Math.floor(totalSeconds / 3600);
                                        const m = Math.floor((totalSeconds % 3600) / 60);
                                        const s = totalSeconds % 60;
                                        return h > 0 ? `${h}h ${m}m ${s}s` : `${m}m ${s}s`;
                                    })()}
                                </p>
                            </div>
                        </div>
                    </CardContent>
                </Card>
            </div>

            {/* Test Solution / Graded Answers */}
            <div className="space-y-4">
                <h3 className="text-xl font-bold flex items-center gap-2">
                    <Target className="h-5 w-5 text-primary" />
                    Test Solution
                </h3>

                {attempt.graded_answers && attempt.graded_answers.length > 0 ? (
                    <div className="space-y-4">
                        {attempt.graded_answers.map((answer, index) => (
                            <Card key={index} className={cn(
                                "border-l-4",
                                answer.is_correct ? "border-l-green-500" : "border-l-destructive"
                            )}>
                                <CardHeader className="py-4">
                                    <div className="flex items-start justify-between gap-4">
                                        <div className="space-y-1">
                                            <p className="text-xs font-semibold text-muted-foreground">QUESTION {index + 1}</p>
                                            <p className="font-medium">{answer.question_text}</p>
                                        </div>
                                        <Badge variant={answer.is_correct ? "outline" : "destructive"} className={cn(
                                            "shrink-0",
                                            answer.is_correct ? "text-green-600 border-green-200 bg-green-50" : ""
                                        )}>
                                            {answer.points_earned} / {answer.max_points} pts
                                        </Badge>
                                    </div>
                                </CardHeader>
                                <CardContent className="space-y-3 pb-4">
                                    <div className="grid gap-4 md:grid-cols-2">
                                        <div className="space-y-1">
                                            <p className="text-xs text-muted-foreground uppercase">Candidate Answer</p>
                                            <div className="p-3 rounded-md bg-muted text-sm border whitespace-pre-wrap">
                                                {typeof answer.candidate_answer === 'object' ? JSON.stringify(answer.candidate_answer) : (answer.candidate_answer || 'No answer')}
                                            </div>
                                        </div>
                                        <div className="space-y-1">
                                            <p className="text-xs text-muted-foreground uppercase">Correct Answer</p>
                                            <div className="p-3 rounded-md bg-green-500/5 text-sm border border-green-500/10 text-green-700 dark:text-green-400 whitespace-pre-wrap font-medium">
                                                {typeof answer.correct_answer === 'object' ? JSON.stringify(answer.correct_answer) : (answer.correct_answer || 'N/A')}
                                            </div>
                                        </div>
                                    </div>
                                    {answer.explanation && (
                                        <div className="p-3 rounded-md bg-blue-500/5 text-xs border border-blue-500/10 text-blue-700 dark:text-blue-300">
                                            <span className="font-bold mr-1 italic">Note:</span> {answer.explanation}
                                        </div>
                                    )}
                                </CardContent>
                            </Card>
                        ))}
                    </div>
                ) : (
                    <Card className="border-dashed">
                        <CardContent className="flex flex-col items-center justify-center py-12 text-center">
                            <FileText className="h-12 w-12 text-muted-foreground/30 mb-4" />
                            <p className="text-muted-foreground">No graded answers available for this attempt.</p>
                            <p className="text-sm text-muted-foreground/60">{attempt.status === 'pending' ? 'The candidate has not started the test yet.' : 'Results may still be processing.'}</p>
                        </CardContent>
                    </Card>
                )}
            </div>
        </div>
    );
}
