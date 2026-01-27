'use client';

import { useQuery } from '@tanstack/react-query';
import { apiFetch } from '@/lib/api';
import { ExternalVacancy, ExternalCompany, ExternalVacancyListResponse, Candidate } from '@/types/api';
import { Card, CardContent, CardHeader, CardFooter } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { MapPin, Building2, Flame, ExternalLink, Calendar, Users } from 'lucide-react';
import { format } from 'date-fns';
import { useState, useMemo } from 'react';
import { useTranslation } from '@/lib/i18n-context';
import { enUS, ru as ruLocale } from 'date-fns/locale';
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle
} from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import { Button } from "@/components/ui/button";

export default function VacanciesPage() {
    const { t, language } = useTranslation();
    const dateLocale = language === 'ru' ? ruLocale : enUS;
    const { data, isLoading, error } = useQuery({
        queryKey: ['external-vacancies'],
        queryFn: () => apiFetch<ExternalVacancyListResponse>('/api/integration/external-vacancies'),
    });

    const [selectedVacancy, setSelectedVacancy] = useState<ExternalVacancy | null>(null);
    const [viewCandidatesVacancy, setViewCandidatesVacancy] = useState<ExternalVacancy | null>(null);

    // Fetch candidates for selected vacancy
    const { data: candidates, isLoading: candidatesLoading } = useQuery({
        queryKey: ['vacancy-candidates', viewCandidatesVacancy?.id],
        queryFn: () => apiFetch<Candidate[]>(`/api/vacancy/${viewCandidatesVacancy?.id}/candidates`),
        enabled: !!viewCandidatesVacancy,
    });

    const companyMap = useMemo(() => {
        const map = new Map<number, ExternalCompany>();
        if (data?.companies) {
            data.companies.forEach(c => map.set(c.id, c));
        }
        return map;
    }, [data]);

    const loadingSkeleton = (
        <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            {[1, 2, 3, 4, 5, 6, 7, 8].map((i) => (
                <Card key={i} className="h-32">
                    <CardHeader className="p-4 pb-2 space-y-2">
                        <div className="flex justify-between">
                            <Skeleton className="h-3 w-20" />
                            <Skeleton className="h-4 w-8" />
                        </div>
                        <Skeleton className="h-4 w-full" />
                        <Skeleton className="h-4 w-2/3" />
                    </CardHeader>
                    <CardContent className="p-4 pt-2">
                        <Skeleton className="h-3 w-24" />
                    </CardContent>
                </Card>
            ))}
        </div>
    );

    if (isLoading) {
        return (
            <div className="space-y-6">
                <div className="flex items-start justify-between">
                    <div className="space-y-1">
                        <Skeleton className="h-8 w-48" />
                        <Skeleton className="h-4 w-64" />
                    </div>
                </div>
                {loadingSkeleton}
            </div>
        );
    }

    if (error) {
        return (
            <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-4">
                <p className="text-destructive font-medium">{t('common.error')}</p>
                <p className="text-sm text-destructive/80 mt-1">{error.message}</p>
            </div>
        );
    }

    const getCompany = (id: number | null) => id ? companyMap.get(id) : null;

    return (
        <div className="space-y-6">
            <div className="flex items-start justify-between">
                <div className="space-y-1">
                    <h3 className="text-2xl font-bold tracking-tight">{t('dashboard.vacancies.title')}</h3>
                    <p className="text-muted-foreground flex items-center gap-2 text-xs">
                        <ExternalLink className="h-3 w-3" />
                        job.koinotinav.tj
                    </p>
                </div>
                <Badge variant="secondary" className="text-xs">
                    {data?.vacancies.length || 0} {t('dashboard.vacancies.active')}
                </Badge>
            </div>

            <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
                {data?.vacancies.map((vacancy) => {
                    const company = getCompany(vacancy.company_id);
                    return (
                        <Card
                            key={vacancy.id}
                            className="cursor-pointer premium-hover group overflow-hidden border-muted-foreground/10"
                            onClick={() => setSelectedVacancy(vacancy)}
                        >
                            <CardHeader className="p-3 pb-1.5 space-y-1.5">
                                <div className="flex items-start justify-between gap-2">
                                    <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
                                        <Building2 className="h-3 w-3 shrink-0" />
                                        <span className="truncate max-w-[120px] font-medium">{company?.title || vacancy.direction}</span>
                                    </div>
                                    {vacancy.hot && (
                                        <Badge variant="destructive" className="h-4 px-1 shrink-0 font-bold shadow-sm">
                                            <Flame className="h-2.5 w-2.5 fill-current" />
                                        </Badge>
                                    )}
                                </div>
                                <div
                                    className="font-bold leading-tight line-clamp-2 group-hover:text-primary transition-colors text-base text-foreground [&_*]:!text-foreground [&_*]:!text-inherit"
                                    dangerouslySetInnerHTML={{ __html: vacancy.title }}
                                />
                            </CardHeader>
                            <CardContent className="p-3 pt-1.5">
                                <div className="flex flex-col gap-1 text-[11px] text-muted-foreground">
                                    <div className="flex items-center gap-1.5">
                                        <MapPin className="h-3 w-3 shrink-0" />
                                        <span>{vacancy.city}</span>
                                    </div>
                                    <div className="flex items-center gap-2 opacity-70 mt-0.5 justify-between">
                                        <div className="flex items-center gap-1.5">
                                            <Calendar className="h-3 w-3 shrink-0" />
                                            <span>{format(new Date(vacancy.created_at), 'dd MMM yyyy', { locale: dateLocale })}</span>
                                        </div>
                                        <span className="text-[10px] font-mono">#{vacancy.id}</span>
                                    </div>
                                </div>
                            </CardContent>
                            <CardFooter className="p-3 pt-0">
                                <Button
                                    size="sm"
                                    variant="secondary"
                                    className="w-full gap-2"
                                    onClick={(e) => {
                                        e.stopPropagation();
                                        setViewCandidatesVacancy(vacancy);
                                    }}
                                >
                                    <Users className="h-4 w-4" />
                                    {t('dashboard.vacancies.applied_users')}
                                </Button>
                            </CardFooter>
                        </Card>
                    );
                })}

                {data?.vacancies.length === 0 && (
                    <div className="col-span-full">
                        <Card className="border-dashed">
                            <CardContent className="flex flex-col items-center justify-center h-48 text-center">
                                <ExternalLink className="h-10 w-10 text-muted-foreground/50 mb-3" />
                                <p className="text-muted-foreground font-medium">{t('dashboard.vacancies.no_vacancies')}</p>
                                <p className="text-xs text-muted-foreground/70 mt-1">
                                    {t('dashboard.vacancies.no_vacancies')}
                                </p>
                            </CardContent>
                        </Card>
                    </div>
                )}
            </div>

            <Dialog open={!!selectedVacancy} onOpenChange={(open) => !open && setSelectedVacancy(null)}>
                <DialogContent className="max-w-2xl max-h-[85vh] overflow-y-auto">
                    {selectedVacancy && (
                        <>
                            <DialogHeader className="mb-4 space-y-3 text-left">
                                <div>
                                    <div className="flex items-center gap-2 text-sm text-muted-foreground mb-1">
                                        <Badge variant="outline" className="font-mono text-xs">#{selectedVacancy.id}</Badge>
                                        {selectedVacancy.company_id && (
                                            <>
                                                <Badge variant="outline" className="font-medium">{getCompany(selectedVacancy.company_id)?.title}</Badge>
                                                <span>•</span>
                                            </>
                                        )}
                                        <span className="flex items-center gap-1"><Building2 className="h-3 w-3" /> {selectedVacancy.direction}</span>
                                    </div>
                                    <DialogTitle
                                        className="text-2xl mt-2 select-text [&_*]:!text-foreground"
                                        dangerouslySetInnerHTML={{ __html: selectedVacancy.title }}
                                    />
                                    <div className="flex items-center gap-4 mt-2 text-sm text-muted-foreground">
                                        <span className="flex items-center gap-1.5">
                                            <MapPin className="h-3 w-3" /> {selectedVacancy.city}
                                        </span>
                                        <span className="flex items-center gap-1.5">
                                            <Calendar className="h-3 w-3" /> {format(new Date(selectedVacancy.created_at), 'PPP', { locale: dateLocale })}
                                        </span>
                                    </div>
                                </div>
                            </DialogHeader>

                            <div className="prose prose-sm dark:prose-invert max-w-none select-text [&_*]:!text-foreground/90">
                                <div dangerouslySetInnerHTML={{ __html: selectedVacancy.content }} />
                            </div>
                        </>
                    )}
                </DialogContent>
            </Dialog>

            <Dialog open={!!viewCandidatesVacancy} onOpenChange={(open) => !open && setViewCandidatesVacancy(null)}>
                <DialogContent className="max-w-xl max-h-[85vh] overflow-y-auto">
                    <DialogHeader>
                        <DialogTitle>{t('dashboard.vacancies.applied_users')}: {viewCandidatesVacancy?.title.replace(/<[^>]*>?/gm, "")}</DialogTitle>
                    </DialogHeader>

                    {candidatesLoading ? (
                        <div className="space-y-4 py-4">
                            <Skeleton className="h-12 w-full" />
                            <Skeleton className="h-12 w-full" />
                            <Skeleton className="h-12 w-full" />
                        </div>
                    ) : !candidates || candidates.length === 0 ? (
                        <div className="py-8 text-center text-muted-foreground">
                            {t('dashboard.vacancies.no_candidates_applied')}
                        </div>
                    ) : (
                        <div className="grid gap-3">
                            <div className="text-sm text-muted-foreground pb-2">{t('dashboard.attempts.total')}: {candidates.length}</div>
                            {candidates.map((c) => (
                                <div key={c.id} className="flex items-center justify-between p-3 border rounded-lg bg-card">
                                    <div className="space-y-1">
                                        <div className="font-medium">{c.name}</div>
                                        <div className="text-xs text-muted-foreground flex gap-2">
                                            <span>{c.email}</span>
                                            {c.phone && <span>• {c.phone}</span>}
                                        </div>
                                    </div>
                                    <Button size="sm" variant="outline" asChild className="shrink-0">
                                        <a href={`/dashboard/candidates?highlight=${c.id}`}>
                                            {t('dashboard.vacancies.profile')}
                                        </a>
                                    </Button>
                                </div>
                            ))}
                        </div>
                    )}
                </DialogContent>
            </Dialog>
        </div>
    );
}
