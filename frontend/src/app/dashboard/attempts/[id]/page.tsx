'use client';

import { useParams, useRouter } from 'next/navigation';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from '@/lib/api';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { toast } from 'sonner';
import {
    ArrowLeft,
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
    BarChart3,
    Link2,
    Download,
    Star,
    Presentation
}
    from 'lucide-react';
import { useTranslation } from '@/lib/i18n-context';
import { format } from 'date-fns';
import { enUS, ru as ruLocale } from 'date-fns/locale';
import { cn } from "@/lib/utils";
import { useState } from 'react';

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
    needs_review?: boolean;
}

interface AttemptDetails {
    id: string;
    test: {
        id: string;
        title: string;
        test_type?: 'question_based' | 'presentation';
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
    presentation_submission_link?: string;
    presentation_submission_file_path?: string;
    presentation_grade?: number;
    presentation_grade_comment?: string;
    metadata?: any;
}

export default function AttemptDetailsPage() {
    const { t, language } = useTranslation();
    const params = useParams();
    const router = useRouter();
    const queryClient = useQueryClient();
    const id = params.id as string;
    const dateLocale = language === 'ru' ? ruLocale : enUS;

    const [gradeInput, setGradeInput] = useState<string>('');
    const [commentInput, setCommentInput] = useState<string>('');

    const { data: attempt, isLoading, error } = useQuery({
        queryKey: ['test-attempt', id],
        queryFn: () => apiFetch<AttemptDetails>(`/api/integration/test-attempts/${id}`),
    });

    const gradeMutation = useMutation({
        mutationFn: (payload: { grade: number; comment?: string }) =>
            apiFetch(`/api/integration/test-attempts/${id}/grade`, {
                method: 'POST',
                body: JSON.stringify(payload),
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ['test-attempt', id] });
            toast.success(t('dashboard.tests_new.toasts.success'));
        },
        onError: (err) => {
            toast.error(`${t('dashboard.tests_new.toasts.error')}: ${err.message}`);
        },
    });

    const gradeAnswerMutation = useMutation({
        mutationFn: (payload: { question_id: number; is_correct: boolean }) =>
            apiFetch(`/api/integration/test-attempts/${id}/grade-answer`, {
                method: 'POST',
                body: JSON.stringify(payload),
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ['test-attempt', id] });
            toast.success(t('common.success'));
        },
        onError: (err) => {
            toast.error(`${t('common.error')}: ${err.message}`);
        },
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
                <Button variant="ghost" size="icon" onClick={() => router.back()}>
                    <ArrowLeft className="h-4 w-4" />
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
            in_progress: { variant: 'secondary', icon: Clock, label: t('dashboard.attempts.statuses.in_progress') },
            active: { variant: 'secondary', icon: Clock, label: t('dashboard.attempts.statuses.active') },
            expired: { variant: 'outline', icon: XCircle, label: t('dashboard.attempts.statuses.expired') },
            pending: { variant: 'secondary', icon: Clock, label: t('dashboard.invites.statuses.pending') },
            needs_review: { variant: 'outline', icon: AlertCircle, label: t('dashboard.attempts.statuses.needs_review') },
        };

        const config = variants[status] || { variant: 'outline', icon: AlertCircle, label: t(`dashboard.attempts.statuses.${status}`) || status };
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
                <Button variant="ghost" size="icon" onClick={() => router.back()}>
                    <ArrowLeft className="h-4 w-4" />
                </Button>
                <div className="flex items-center gap-3">
                    {getStatusBadge(attempt.status)}
                    {attempt.passed !== undefined && (
                        <Badge variant={attempt.passed ? "default" : "destructive"} className={attempt.passed ? "bg-green-600" : ""}>
                            {attempt.passed ? t('dashboard.attempts.labels.passed') : t('dashboard.attempts.labels.failed')}
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
                                <span>{t('dashboard.attempts.labels.external_id')}: {attempt.candidate.telegram_id}</span>
                            </div>
                        )}
                    </CardContent>
                </Card>

                {/* Test Results Summary */}
                <Card className="md:col-span-2 premium-hover border-primary/10">
                    <CardHeader>
                        <CardTitle className="text-lg flex items-center gap-2">
                            <BarChart3 className="h-5 w-5 text-primary" />
                            {t('dashboard.attempts.card.result')} {t('dashboard.attempts.labels.details')}
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
                                        const parts = [];
                                        if (h > 0) parts.push(`${h}${t('common.unit_h')}`);
                                        if (m > 0 || h > 0) parts.push(`${m}${t('common.unit_m')}`);
                                        parts.push(`${s}${t('common.unit_s')}`);
                                        return parts.join(' ');
                                    })()}
                                </p>
                            </div>
                        </div>
                    </CardContent>
                </Card>
            </div>


            {/* Presentation Grading Display */}
            {attempt.test.test_type === 'presentation' && (
                <div className="space-y-6">
                    <div className="flex items-center justify-between border-b pb-4">
                        <h3 className="text-2xl font-bold flex items-center gap-3">
                            <Presentation className="h-6 w-6 text-purple-600" />
                            {t('dashboard.attempts.presentation.submission_heading')}
                        </h3>
                        {attempt.presentation_grade != null ? (
                            <Badge className="bg-green-600 hover:bg-green-600 gap-1">
                                <CheckCircle className="h-3 w-3" />
                                {t('dashboard.attempts.presentation.graded')}
                            </Badge>
                        ) : (
                            <Badge variant="outline" className="gap-1 border-amber-500 text-amber-500">
                                <AlertCircle className="h-3 w-3" />
                                {t('dashboard.attempts.presentation.needs_review')}
                            </Badge>
                        )}
                    </div>

                    <div className="grid gap-6 md:grid-cols-2">
                        {/* Submission Card */}
                        <Card>
                            <CardHeader>
                                <CardTitle className="text-lg">
                                    {t('dashboard.attempts.presentation.submission_file')} / {t('dashboard.attempts.presentation.submission_link')}
                                </CardTitle>
                            </CardHeader>
                            <CardContent className="space-y-4">
                                {attempt.presentation_submission_link ? (
                                    <div className="p-4 rounded-lg bg-muted/50 border flex items-center gap-3">
                                        <div className="h-10 w-10 rounded-full bg-blue-100 dark:bg-blue-900/30 flex items-center justify-center shrink-0">
                                            <Link2 className="h-5 w-5 text-blue-600 dark:text-blue-400" />
                                        </div>
                                        <div className="overflow-hidden">
                                            <p className="text-xs text-muted-foreground uppercase font-bold tracking-wider">{t('dashboard.attempts.presentation.label_link')}</p>
                                            <a href={attempt.presentation_submission_link} target="_blank" rel="noopener noreferrer" className="text-sm font-medium hover:underline truncate block text-primary">
                                                {attempt.presentation_submission_link}
                                            </a>
                                        </div>
                                    </div>
                                ) : null}

                                {attempt.presentation_submission_file_path ? (
                                    <div className="p-4 rounded-lg bg-muted/50 border flex items-center gap-3">
                                        <div className="h-10 w-10 rounded-full bg-green-100 dark:bg-green-900/30 flex items-center justify-center shrink-0">
                                            <FileText className="h-5 w-5 text-green-600 dark:text-green-400" />
                                        </div>
                                        <div className="flex-1 overflow-hidden">
                                            <p className="text-xs text-muted-foreground uppercase font-bold tracking-wider">{t('dashboard.attempts.presentation.label_file')}</p>
                                            <p className="text-sm font-medium truncate">{attempt.presentation_submission_file_path.split('/').pop()}</p>
                                        </div>
                                        <a href={`/${attempt.presentation_submission_file_path}`} target="_blank" rel="noopener noreferrer">
                                            <Button size="sm" variant="outline" className="gap-2">
                                                <Download className="h-4 w-4" />
                                                {t('dashboard.attempts.presentation.download_file')}
                                            </Button>
                                        </a>
                                    </div>
                                ) : null}

                                {!attempt.presentation_submission_link && !attempt.presentation_submission_file_path && (
                                    <div className="p-8 text-center text-muted-foreground bg-muted/20 rounded-lg border border-dashed">
                                        <AlertCircle className="h-8 w-8 mx-auto mb-2 opacity-50" />
                                        <p>{t('dashboard.attempts.presentation.missing_submission')}</p>
                                    </div>
                                )}
                            </CardContent>
                        </Card>

                        {/* Grading Card */}
                        <Card>
                            <CardHeader>
                                <CardTitle className="text-lg flex items-center gap-2">
                                    {t('dashboard.attempts.presentation.grade')}
                                </CardTitle>
                                <CardDescription>
                                    {t('dashboard.attempts.presentation.evaluate_desc')}
                                </CardDescription>
                            </CardHeader>
                            <CardContent className="space-y-4">
                                {attempt.presentation_grade !== null ? (
                                    <div className="space-y-4">
                                        <div className="p-4 rounded-lg bg-green-500/5 border border-green-500/20 space-y-3">
                                            <div className="flex items-center justify-between">
                                                <Label className="text-muted-foreground">{t('dashboard.attempts.presentation.score_label')}</Label>
                                                <Badge className="bg-green-600 font-bold">{attempt.presentation_grade} / 100</Badge>
                                            </div>
                                            <div className="space-y-1">
                                                <Label className="text-muted-foreground">{t('dashboard.attempts.presentation.grade_comment')}</Label>
                                                <div className="text-sm p-3 rounded bg-background border italic">
                                                    {attempt.presentation_grade_comment || t('test.no_comment')}
                                                </div>
                                            </div>
                                        </div>
                                        <Button className="w-full" disabled>
                                            <CheckCircle className="h-4 w-4 mr-2" />
                                            {t('dashboard.attempts.presentation.graded')}
                                        </Button>
                                    </div>
                                ) : (
                                    <>
                                        <div className="space-y-2">
                                            <Label>{t('dashboard.attempts.presentation.score_label')}</Label>
                                            <Input
                                                type="number"
                                                min="0"
                                                max="100"
                                                placeholder={t('dashboard.attempts.presentation.score_placeholder')}
                                                value={gradeInput}
                                                onChange={(e) => setGradeInput(e.target.value)}
                                            />
                                        </div>
                                        <div className="space-y-2">
                                            <Label>{t('dashboard.attempts.presentation.grade_comment')}</Label>
                                            <Textarea
                                                placeholder={t('dashboard.attempts.presentation.grade_placeholder')}
                                                value={commentInput}
                                                onChange={(e) => setCommentInput(e.target.value)}
                                                className="h-32"
                                            />
                                        </div>
                                        <Button
                                            className="w-full"
                                            onClick={() => gradeMutation.mutate({
                                                grade: parseFloat(gradeInput || '0'),
                                                comment: commentInput
                                            })}
                                            disabled={gradeMutation.isPending || !gradeInput}
                                        >
                                            {gradeMutation.isPending ? t('common.saving') : t('dashboard.attempts.presentation.save_grade')}
                                        </Button>
                                    </>
                                )}
                            </CardContent>
                        </Card>
                    </div>
                </div>
            )}

            {attempt.test.test_type !== 'presentation' && (
                <div className="space-y-6">
                    <div className="flex items-center justify-between border-b pb-4">
                        <h3 className="text-2xl font-bold flex items-center gap-3">
                            <Target className="h-6 w-6 text-primary" />
                            {t('dashboard.attempts.solution.title')}
                        </h3>
                        <div className="text-sm text-muted-foreground bg-muted px-3 py-1 rounded-full border">
                            {attempt.graded_answers?.length || 0} {t('dashboard.attempts.solution.question').toLowerCase()}
                        </div>
                    </div>

                    {attempt.graded_answers && attempt.graded_answers.length > 0 ? (
                        <div className="grid gap-6">
                            {attempt.graded_answers.map((answer: GradedAnswer, index: number) => (
                                <Card key={index} className={cn(
                                    "overflow-hidden border-none shadow-lg premium-hover",
                                    answer.is_correct ? "bg-green-500/5 ring-1 ring-green-500/20" : "bg-destructive/5 ring-1 ring-destructive/20"
                                )}>
                                    <CardHeader className="pb-3 border-b border-border/50 bg-background/50">
                                        <div className="flex items-start justify-between gap-4">
                                            <div className="space-y-2">
                                                <div className="flex items-center gap-2">
                                                    <Badge variant="outline" className="font-mono text-[10px] uppercase tracking-wider bg-background/50">
                                                        {t('dashboard.attempts.solution.question')} {index + 1}
                                                    </Badge>
                                                    {answer.needs_review ? (
                                                        <Badge variant="outline" className="gap-1 text-[10px] border-amber-500 text-amber-500">
                                                            <AlertCircle className="h-3 w-3" />
                                                            {t('dashboard.attempts.statuses.needs_review')}
                                                        </Badge>
                                                    ) : answer.is_correct ? (
                                                        <Badge className="bg-green-600 hover:bg-green-600 gap-1 text-[10px]">
                                                            <CheckCircle className="h-3 w-3" />
                                                            {t('dashboard.attempts.labels.passed')}
                                                        </Badge>
                                                    ) : (
                                                        <Badge variant="destructive" className="gap-1 text-[10px]">
                                                            <XCircle className="h-3 w-3" />
                                                            {t('dashboard.attempts.labels.failed')}
                                                        </Badge>
                                                    )}
                                                </div>
                                                <h4 className="text-lg font-bold leading-tight text-foreground/90">
                                                    {answer.question_text || (t('dashboard.attempts.solution.question') + " " + (index + 1))}
                                                </h4>
                                            </div>
                                            <div className="flex flex-col gap-2 items-end">
                                                <div className="bg-background/80 backdrop-blur-sm border rounded-lg p-2 text-center min-w-[80px] shadow-sm">
                                                    <p className="text-[10px] text-muted-foreground uppercase font-bold tracking-tighter">{t('dashboard.attempts.labels.score')}</p>
                                                    <div className="flex items-baseline justify-center gap-0.5">
                                                        <span className={cn("text-lg font-black", answer.is_correct ? "text-green-600" : (answer.needs_review ? "text-amber-600" : "text-destructive"))}>
                                                            {answer.points_earned}
                                                        </span>
                                                        <span className="text-muted-foreground text-xs">/ {answer.max_points}</span>
                                                    </div>
                                                </div>
                                                {answer.type === 'short_answer' && (
                                                    <div className="flex gap-2">
                                                        <Button
                                                            size="sm"
                                                            variant={answer.is_correct && !answer.needs_review ? "default" : "outline"}
                                                            className={cn("h-7 px-2 text-[10px]", answer.is_correct && !answer.needs_review ? "bg-green-600 hover:bg-green-700" : "")}
                                                            onClick={() => gradeAnswerMutation.mutate({ question_id: answer.question_id, is_correct: true })}
                                                            disabled={gradeAnswerMutation.isPending}
                                                        >
                                                            {t('dashboard.attempts.solution.mark_correct')}
                                                        </Button>
                                                        <Button
                                                            size="sm"
                                                            variant={!answer.is_correct && !answer.needs_review ? "destructive" : "outline"}
                                                            className="h-7 px-2 text-[10px]"
                                                            onClick={() => gradeAnswerMutation.mutate({ question_id: answer.question_id, is_correct: false })}
                                                            disabled={gradeAnswerMutation.isPending}
                                                        >
                                                            {t('dashboard.attempts.solution.mark_incorrect')}
                                                        </Button>
                                                    </div>
                                                )}
                                            </div>
                                        </div>
                                    </CardHeader>
                                    <CardContent className="pt-6 space-y-6">
                                        <div className="grid gap-6 md:grid-cols-2">
                                            {/* Candidate Answer Box */}
                                            <div className="space-y-3">
                                                <div className="flex items-center gap-2 px-1">
                                                    <User className="h-3.5 w-3.5 text-muted-foreground" />
                                                    <span className="text-sm font-bold text-muted-foreground uppercase tracking-wide">
                                                        {t('dashboard.attempts.solution.candidate_answer')}
                                                    </span>
                                                </div>
                                                <div className={cn(
                                                    "relative p-4 rounded-xl border-2 text-sm transition-all shadow-inner min-h-[100px] flex items-center",
                                                    answer.is_correct
                                                        ? "bg-green-500/5 border-green-500/20 text-green-900 dark:text-green-100"
                                                        : "bg-destructive/5 border-destructive/20 text-destructive dark:text-red-200"
                                                )}>
                                                    <div className="absolute top-3 right-3 opacity-20">
                                                        {answer.is_correct ? <CheckCircle className="h-5 w-5" /> : <XCircle className="h-5 w-5" />}
                                                    </div>
                                                    <p className="font-mono whitespace-pre-wrap break-words w-full">
                                                        {typeof answer.candidate_answer === 'object' ? JSON.stringify(answer.candidate_answer) : (answer.candidate_answer || t('dashboard.attempts.solution.no_answer'))}
                                                    </p>
                                                </div>
                                            </div>

                                            {/* Correct Answer Box */}
                                            <div className="space-y-3">
                                                <div className="flex items-center gap-2 px-1">
                                                    <CheckCircle className="h-3.5 w-3.5 text-green-600" />
                                                    <span className="text-sm font-bold text-green-600 uppercase tracking-wide">
                                                        {t('dashboard.attempts.solution.correct_answer')}
                                                    </span>
                                                </div>
                                                <div className="p-4 rounded-xl border-2 border-green-500/30 bg-green-500/10 text-green-800 dark:text-green-200 text-sm shadow-inner min-h-[100px] flex items-center overflow-hidden">
                                                    <p className="font-mono font-bold whitespace-pre-wrap break-words w-full">
                                                        {typeof answer.correct_answer === 'object' ? JSON.stringify(answer.correct_answer) : (answer.correct_answer || t('common.not_applicable'))}
                                                    </p>
                                                </div>
                                            </div>
                                        </div>

                                        {answer.explanation && (
                                            <div className="flex gap-3 p-4 rounded-xl bg-blue-500/5 border border-blue-500/10 text-blue-700 dark:text-blue-300">
                                                <AlertCircle className="h-5 w-5 shrink-0 mt-0.5" />
                                                <div className="space-y-1">
                                                    <p className="text-xs font-bold uppercase tracking-wider opacity-80">{t('dashboard.attempts.solution.note')}</p>
                                                    <p className="text-sm leading-relaxed">{answer.explanation}</p>
                                                </div>
                                            </div>
                                        )}
                                    </CardContent>
                                </Card>
                            ))}
                        </div>
                    ) : (
                        <Card className="border-dashed bg-muted/30">
                            <CardContent className="flex flex-col items-center justify-center py-16 text-center">
                                <div className="bg-background p-4 rounded-full shadow-sm mb-6">
                                    <FileText className="h-10 w-10 text-muted-foreground/30" />
                                </div>
                                <h4 className="text-lg font-semibold mb-2">{t('dashboard.attempts.solution.no_answers')}</h4>
                                <p className="text-sm text-muted-foreground max-w-xs mx-auto">
                                    {attempt.status === 'pending' ? t('dashboard.attempts.solution.no_answers_pending') : t('dashboard.attempts.solution.no_answers_processing')}
                                </p>
                            </CardContent>
                        </Card>
                    )}
                </div>
            )}
        </div>
    );
}
