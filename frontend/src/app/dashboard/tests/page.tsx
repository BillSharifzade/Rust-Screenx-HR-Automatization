'use client';

import { formatText } from '@/lib/utils';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from '@/lib/api';
import { Test } from '@/types/api';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import {
    Plus, Clock, Trash2, Edit, Send,
    Calendar, ListChecks, AlertCircle
} from 'lucide-react';
import { format } from 'date-fns';
import { enUS, ru as ruLocale } from 'date-fns/locale';
import Link from 'next/link';
import { toast } from 'sonner';
import {
    AlertDialog,
    AlertDialogAction,
    AlertDialogCancel,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle,
    AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { useRouter } from 'next/navigation';
import { useTranslation } from '@/lib/i18n-context';

export default function TestsPage() {
    const { t, language } = useTranslation();
    const dateLocale = language === 'ru' ? ruLocale : enUS;
    const queryClient = useQueryClient();
    const router = useRouter();

    const { data, isLoading, error } = useQuery({
        queryKey: ['tests'],
        queryFn: () => apiFetch<{ items: Test[] }>('/api/integration/tests'),
    });

    const deleteMutation = useMutation({
        mutationFn: (id: string) => apiFetch(`/api/integration/tests/${id}`, { method: 'DELETE' }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ['tests'] });
            toast.success(t('common.success'));
        },
        onError: (err) => {
            toast.error(`${t('common.error')}: ${err.message}`);
        },
    });

    const handleDelete = (id: string) => {
        deleteMutation.mutate(id);
    };

    if (isLoading) {
        return <div>{t('common.loading')}</div>;
    }

    if (error) {
        return <div className="text-destructive">{t('common.error')}: {error.message}</div>;
    }

    return (
        <div className="space-y-6">
            <div className="flex items-center justify-between">
                <div>
                    <h3 className="text-2xl font-bold tracking-tight">{t('dashboard.tests.title')}</h3>
                    <p className="text-muted-foreground">
                        {t('dashboard.tests.subtitle')}
                    </p>
                </div>
                <div className="flex gap-2">
                    <Link href="/dashboard/tests/presentation/new">
                        <Button variant="outline">
                            <Plus className="mr-2 h-4 w-4" />
                            {t('dashboard.attempts.presentation.create_title')}
                        </Button>
                    </Link>
                    <Link href="/dashboard/tests/new">
                        <Button>
                            <Plus className="mr-2 h-4 w-4" />
                            {t('dashboard.tests.create')}
                        </Button>
                    </Link>
                </div>
            </div>

            <div className="grid gap-6">
                {data?.items?.map((test) => (
                    <Card key={test.id} className="overflow-hidden premium-hover">
                        <CardHeader className="flex flex-row items-start justify-between space-y-0 pb-2">
                            <div className="space-y-2">
                                <div className="flex items-center gap-2">
                                    <CardTitle className="text-xl font-semibold text-primary">
                                        <Link href={`/dashboard/tests/${test.id}`} className="hover:underline">
                                            {test.title}
                                        </Link>
                                    </CardTitle>
                                    {test.is_active === false && (
                                        <Badge variant="secondary" className="text-xs">{t('dashboard.tests.archive')}</Badge>
                                    )}
                                    {test.test_type === 'presentation' && (
                                        <Badge variant="default" className="bg-purple-600 hover:bg-purple-700 text-xs">
                                            {t('dashboard.attempts.presentation.title')}
                                        </Badge>
                                    )}
                                </div>
                                <p className="text-sm text-muted-foreground line-clamp-2 max-w-3xl whitespace-pre-wrap">
                                    {formatText(test.description) || t('dashboard.tests.no_description')}
                                </p>
                            </div>
                            <Badge variant={test.passing_score >= 80 ? "default" : "secondary"}>
                                {t('dashboard.tests.passing_score')}: {test.passing_score}%
                            </Badge>
                        </CardHeader>
                        <CardContent className="py-4">
                            <div className="flex flex-col md:flex-row md:items-center justify-between gap-6">
                                <div className="space-y-4 flex-1">
                                    <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 text-sm text-muted-foreground">
                                        <div className="flex items-center gap-2">
                                            <Clock className="h-4 w-4 text-primary/70" />
                                            <span>
                                                {test.test_type === 'presentation'
                                                    ? `${test.duration_minutes / 60} ${t('dashboard.attempts.presentation.hours_short') || t('dashboard.attempts.presentation.hours')}`
                                                    : `${test.duration_minutes} ${t('test.duration_unit')}`
                                                }
                                            </span>
                                        </div>
                                        <div className="flex items-center gap-2">
                                            <ListChecks className="h-4 w-4 text-primary/70" />
                                            <span>
                                                {test.test_type === 'presentation'
                                                    ? `${Array.isArray(test.presentation_themes) ? test.presentation_themes.length : 0} ${t('dashboard.attempts.presentation.themes')}`
                                                    : `${Array.isArray(test.questions) ? test.questions.length : 0} ${t('dashboard.tests.questions')}`
                                                }
                                            </span>
                                        </div>
                                        <div className="flex items-center gap-2">
                                            <Calendar className="h-4 w-4 text-primary/70" />
                                            <span>{test.created_at ? format(new Date(test.created_at), 'dd.MM.yyyy', { locale: dateLocale }) : '-'}</span>
                                        </div>
                                        <div className="flex items-center gap-2">
                                            <AlertCircle className="h-4 w-4 text-primary/70" />
                                            <span>{test.max_attempts || 1} {t('dashboard.tests.attempts')}</span>
                                        </div>
                                    </div>
                                    <div className="flex flex-wrap gap-2">
                                        {test.shuffle_questions && <Badge variant="outline" className="text-xs">{t('dashboard.tests.shuffle_questions')}</Badge>}
                                        {test.show_results_immediately && <Badge variant="outline" className="text-xs">{t('dashboard.tests.show_results')}</Badge>}
                                    </div>
                                </div>

                                <div className="flex items-center gap-2 min-w-fit mt-4 md:mt-0">
                                    <Link href={`/dashboard/tests/${test.id}`}>
                                        <Button variant="outline" size="sm">
                                            <Edit className="mr-2 h-3.5 w-3.5" />
                                            {t('common.edit')}
                                        </Button>
                                    </Link>

                                    <Button
                                        variant="default"
                                        size="sm"
                                        className="bg-blue-600 hover:bg-blue-700 text-white"
                                        onClick={() => router.push(`/dashboard/invites?test=${test.id}`)}
                                    >
                                        <Send className="mr-2 h-3.5 w-3.5" />
                                        {t('dashboard.tests.send')}
                                    </Button>

                                    <AlertDialog>
                                        <AlertDialogTrigger asChild>
                                            <Button variant="ghost" size="sm" className="text-destructive hover:text-destructive hover:bg-destructive/10">
                                                <Trash2 className="h-4 w-4" />
                                            </Button>
                                        </AlertDialogTrigger>
                                        <AlertDialogContent>
                                            <AlertDialogHeader>
                                                <AlertDialogTitle>{t('dashboard.tests.delete_confirm')}</AlertDialogTitle>
                                                <AlertDialogDescription>
                                                    {t('dashboard.tests.delete_desc')}
                                                </AlertDialogDescription>
                                            </AlertDialogHeader>
                                            <AlertDialogFooter>
                                                <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
                                                <AlertDialogAction onClick={() => handleDelete(test.id)} className="bg-destructive hover:bg-destructive/90">
                                                    {t('common.delete')}
                                                </AlertDialogAction>
                                            </AlertDialogFooter>
                                        </AlertDialogContent>
                                    </AlertDialog>
                                </div>
                            </div>
                        </CardContent>
                    </Card>
                ))}
                {(!data?.items || data.items.length === 0) && (
                    <div className="flex h-32 items-center justify-center rounded-lg border border-dashed">
                        <p className="text-muted-foreground">{t('dashboard.tests.no_tests')}</p>
                    </div>
                )}
            </div>
        </div>
    );
}
