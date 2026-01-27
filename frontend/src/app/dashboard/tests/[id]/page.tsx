'use client';

import { useParams, useRouter } from 'next/navigation';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from '@/lib/api';
import { formatText, FormattedText } from '@/lib/utils';
import { Test } from '@/types/api';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Badge } from '@/components/ui/badge';
import { Skeleton } from '@/components/ui/skeleton';
import { ArrowLeft, Save, Clock, ListChecks, CheckCircle2, Presentation, AlertCircle } from 'lucide-react';
import { toast } from 'sonner';
import Link from 'next/link';
import { useTranslation } from '@/lib/i18n-context';
import { format } from 'date-fns';
import { ru, enUS } from 'date-fns/locale';
import { useState, useEffect } from 'react';

export default function TestDetailPage() {
    const { t, language } = useTranslation();
    const params = useParams();
    const queryClient = useQueryClient();
    const testId = params.id as string;
    const dateLocale = language === 'ru' ? ru : enUS;

    const { data: test, isLoading, error } = useQuery<Test>({
        queryKey: ['test', testId],
        queryFn: () => apiFetch(`/api/integration/tests/${testId}`),
    });

    const [title, setTitle] = useState('');
    const [description, setDescription] = useState('');
    const [instructions, setInstructions] = useState('');
    const [durationMinutes, setDurationMinutes] = useState(30);
    const [passingScore, setPassingScore] = useState(70);

    useEffect(() => {
        if (test) {
            setTitle(test.title);
            setDescription(formatText(test.description));
            setInstructions(formatText(test.instructions));
            setDurationMinutes(Number(test.duration_minutes));
            setPassingScore(Number(test.passing_score));
        }
    }, [test]);

    const updateMutation = useMutation({
        mutationFn: (payload: Partial<Test>) =>
            apiFetch(`/api/integration/tests/${testId}`, {
                method: 'PATCH',
                body: JSON.stringify(payload),
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ['tests'] });
            queryClient.invalidateQueries({ queryKey: ['test', testId] });
            toast.success(t('dashboard.tests_new.toasts.success'));
        },
        onError: (err) => {
            toast.error(`${t('dashboard.tests_new.toasts.error')}: ${err.message}`);
        },
    });

    const handleSave = () => {
        // Validate required fields
        if (!title.trim()) {
            toast.error(t('dashboard.tests_new.toasts.error') + ': ' + t('dashboard.tests_new.labels.title'));
            return;
        }

        if (durationMinutes < 1) {
            toast.error(t('dashboard.tests_new.toasts.error') + ': Duration must be at least 1 minute');
            return;
        }

        if (passingScore < 0 || passingScore > 100) {
            toast.error(t('dashboard.tests_new.toasts.error') + ': Passing score must be between 0 and 100');
            return;
        }

        const payload: Partial<Test> = {
            title: title.trim(),
            description: description || undefined,
            instructions: instructions || undefined,
            duration_minutes: Number(durationMinutes),
            passing_score: Number(passingScore),
        };

        updateMutation.mutate(payload);
    };

    if (isLoading) {
        return (
            <div className="max-w-4xl mx-auto space-y-6">
                <div className="flex items-center gap-4">
                    <Skeleton className="h-10 w-10" />
                    <Skeleton className="h-8 w-64" />
                </div>
                <Skeleton className="h-48 w-full" />
                <Skeleton className="h-48 w-full" />
            </div>
        );
    }

    if (error || !test) {
        return (
            <div className="max-w-4xl mx-auto space-y-6">
                <div className="flex items-center gap-4">
                    <Link href="/dashboard/tests">
                        <Button variant="ghost" size="icon">
                            <ArrowLeft className="h-4 w-4" />
                        </Button>
                    </Link>
                    <div>
                        <h3 className="text-2xl font-bold tracking-tight text-destructive">Test Not Found</h3>
                        <p className="text-muted-foreground">The test you're looking for doesn't exist or has been deleted.</p>
                    </div>
                </div>
            </div>
        );
    }

    const isPresentation = test.test_type === 'presentation';

    return (
        <div className="max-w-4xl mx-auto space-y-6">
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                    <Link href="/dashboard/tests">
                        <Button variant="ghost" size="icon">
                            <ArrowLeft className="h-4 w-4" />
                        </Button>
                    </Link>
                    <div>
                        <div className="flex items-center gap-2">
                            <h3 className="text-2xl font-bold tracking-tight">{test.title}</h3>
                            {isPresentation && (
                                <Badge className="bg-purple-600 hover:bg-purple-700">
                                    <Presentation className="h-3 w-3 mr-1" />
                                    {t('dashboard.attempts.presentation.title')}
                                </Badge>
                            )}
                        </div>
                        <p className="text-muted-foreground">
                            {t('dashboard.tests_new.subtitle')}
                        </p>
                    </div>
                </div>
                <Button onClick={handleSave} disabled={updateMutation.isPending}>
                    <Save className="mr-2 h-4 w-4" />
                    {updateMutation.isPending ? t('common.saving') : t('common.save')}
                </Button>
            </div>

            {/* Test Info Card */}
            <Card>
                <CardHeader>
                    <CardTitle>{t('dashboard.tests_new.details_title')}</CardTitle>
                    <CardDescription>
                        {test.created_at ? format(new Date(test.created_at), 'PPP', { locale: dateLocale }) : '—'}
                    </CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    <div className="space-y-2">
                        <Label htmlFor="title">{t('dashboard.tests_new.labels.title')}</Label>
                        <Input
                            id="title"
                            value={title}
                            onChange={(e) => setTitle(e.target.value)}
                            placeholder={t('dashboard.tests_new.placeholders.title')}
                        />
                    </div>

                    <div className="space-y-2">
                        <Label htmlFor="description">{t('dashboard.tests_new.labels.description')}</Label>
                        <Textarea
                            id="description"
                            value={description}
                            onChange={(e) => setDescription(e.target.value)}
                            placeholder={t('dashboard.tests_new.placeholders.description')}
                        />
                    </div>

                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                        <div className="space-y-2">
                            <Label htmlFor="duration">
                                {isPresentation
                                    ? t('dashboard.attempts.presentation.deadline_hours')
                                    : t('dashboard.tests_new.labels.duration')}
                            </Label>
                            <Input
                                id="duration"
                                type="number"
                                value={isPresentation ? durationMinutes / 60 : durationMinutes}
                                onChange={(e) => {
                                    const val = parseInt(e.target.value) || 0;
                                    setDurationMinutes(isPresentation ? val * 60 : val);
                                }}
                            />
                        </div>

                        <div className="space-y-2">
                            <Label htmlFor="passing_score">{t('dashboard.tests_new.labels.passing_score')}</Label>
                            <Input
                                id="passing_score"
                                type="number"
                                min="0"
                                max="100"
                                value={passingScore}
                                onChange={(e) => setPassingScore(parseFloat(e.target.value) || 0)}
                            />
                        </div>
                    </div>
                </CardContent>
            </Card>

            {/* Stats Card */}
            <Card>
                <CardHeader>
                    <CardTitle>{t('dashboard.tests_new.stats.title')}</CardTitle>
                </CardHeader>
                <CardContent>
                    <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                        <div className="flex flex-col items-center justify-center p-4 rounded-lg bg-muted">
                            <Clock className="h-6 w-6 text-primary mb-2" />
                            <span className="text-2xl font-bold">
                                {isPresentation ? `${test.duration_minutes / 60}h` : `${test.duration_minutes}m`}
                            </span>
                            <span className="text-xs text-muted-foreground">{t('dashboard.tests_new.stats.duration')}</span>
                        </div>
                        <div className="flex flex-col items-center justify-center p-4 rounded-lg bg-muted">
                            <ListChecks className="h-6 w-6 text-primary mb-2" />
                            <span className="text-2xl font-bold">
                                {isPresentation
                                    ? test.presentation_themes?.length || 0
                                    : test.questions?.length || 0}
                            </span>
                            <span className="text-xs text-muted-foreground">
                                {isPresentation ? t('dashboard.tests_new.stats.themes') : t('dashboard.tests_new.stats.questions')}
                            </span>
                        </div>
                        <div className="flex flex-col items-center justify-center p-4 rounded-lg bg-muted">
                            <CheckCircle2 className="h-6 w-6 text-green-600 mb-2" />
                            <span className="text-2xl font-bold">{test.passing_score}%</span>
                            <span className="text-xs text-muted-foreground">{t('dashboard.tests_new.stats.passing_score')}</span>
                        </div>
                        <div className="flex flex-col items-center justify-center p-4 rounded-lg bg-muted">
                            <AlertCircle className="h-6 w-6 text-blue-600 mb-2" />
                            <span className="text-2xl font-bold">{test.max_attempts || 1}</span>
                            <span className="text-xs text-muted-foreground">{t('dashboard.tests_new.stats.max_attempts')}</span>
                        </div>
                    </div>
                </CardContent>
            </Card>

            {/* Presentation Themes */}
            {isPresentation && test.presentation_themes && (
                <Card className="border-purple-200 dark:border-purple-800/50">
                    <CardHeader className="bg-purple-50 dark:bg-purple-900/20">
                        <CardTitle className="flex items-center gap-2 text-purple-700 dark:text-purple-300">
                            <ListChecks className="h-5 w-5" />
                            {t('dashboard.attempts.presentation.themes')}
                        </CardTitle>
                    </CardHeader>
                    <CardContent className="pt-6">
                        <div className="space-y-3">
                            {test.presentation_themes.map((theme: string, i: number) => (
                                <div key={i} className="flex items-start gap-3 p-4 rounded-lg bg-muted/50">
                                    <div className="h-6 w-6 rounded-full bg-purple-600 text-white flex items-center justify-center text-sm font-semibold">
                                        {i + 1}
                                    </div>
                                    <span className="text-base">{theme}</span>
                                </div>
                            ))}
                        </div>
                    </CardContent>
                </Card>
            )}

            {/* Extra Info for Presentation */}
            {isPresentation && test.presentation_extra_info && (
                <Card>
                    <CardHeader>
                        <CardTitle>{t('dashboard.attempts.presentation.extra_info')}</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <FormattedText text={test.presentation_extra_info} className="text-muted-foreground" />
                    </CardContent>
                </Card>
            )}

            {/* Questions for Question-based Tests */}
            {!isPresentation && test.questions && test.questions.length > 0 && (
                <Card>
                    <CardHeader>
                        <CardTitle>{t('dashboard.tests_new.questions_list.title')}</CardTitle>
                        <CardDescription>
                            {test.questions.length} {t('dashboard.tests_new.questions_list.count')}
                        </CardDescription>
                    </CardHeader>
                    <CardContent>
                        <div className="space-y-4">
                            {test.questions.map((q: any, i: number) => (
                                <div key={i} className="p-4 rounded-lg bg-muted/50 border">
                                    <div className="flex items-start gap-3">
                                        <div className="h-6 w-6 rounded-full bg-primary text-primary-foreground flex items-center justify-center text-sm font-semibold shrink-0">
                                            {i + 1}
                                        </div>
                                        <div className="flex-1">
                                            <FormattedText text={q.question} className="font-medium" />
                                            <div className="flex items-center gap-2 mt-2">
                                                <Badge variant="secondary">
                                                    {q.type === 'multiple_choice'
                                                        ? t('dashboard.tests_new.question_card.types.multiple_choice')
                                                        : t('dashboard.tests_new.question_card.types.short_answer')}
                                                </Badge>
                                                <span className="text-sm text-muted-foreground">{q.points} pts</span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            ))}
                        </div>
                    </CardContent>
                </Card>
            )}
        </div>
    );
}
