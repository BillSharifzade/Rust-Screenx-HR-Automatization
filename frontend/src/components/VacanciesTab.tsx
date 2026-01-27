"use client"

import { useState, useMemo } from "react"
import { useQuery } from '@tanstack/react-query'
import { format } from "date-fns"
import { Building2, Flame, MapPin, Calendar as CalendarIconLucide, Search } from "lucide-react"
import { useTranslation } from "@/lib/i18n-context"
import { enUS, ru as ruLocale } from 'date-fns/locale';

import { Card, CardHeader } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Skeleton } from "@/components/ui/skeleton"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"

import { apiFetch } from '@/lib/api'
import { ExternalVacancy, ExternalCompany, ExternalVacancyListResponse } from '@/types/api'

interface VacanciesTabProps {
    onSelectVacancy: (vacancy: ExternalVacancy) => void;
    selectButtonText?: string;
}

export function VacanciesTab({ onSelectVacancy, selectButtonText }: VacanciesTabProps) {
    const { t, language } = useTranslation();
    const dateLocale = language === 'ru' ? ruLocale : enUS;
    const { data, isLoading, error } = useQuery({
        queryKey: ['external-vacancies'],
        queryFn: () => apiFetch<ExternalVacancyListResponse>('/api/external-vacancies'),
    });

    const [selectedVacancy, setSelectedVacancy] = useState<ExternalVacancy | null>(null);
    const [searchQuery, setSearchQuery] = useState("");

    const companyMap = useMemo(() => {
        const map = new Map<number, ExternalCompany>();
        if (data?.companies) {
            data.companies.forEach(c => map.set(c.id, c));
        }
        return map;
    }, [data]);

    const getCompany = (id: number | null) => id ? companyMap.get(id) : null;

    const filteredVacancies = useMemo(() => {
        if (!data?.vacancies) return [];
        if (!searchQuery) return data.vacancies;
        const lowerQuery = searchQuery.toLowerCase();
        return data.vacancies.filter(v =>
            v.title.toLowerCase().includes(lowerQuery) ||
            v.city.toLowerCase().includes(lowerQuery) ||
            v.direction.toLowerCase().includes(lowerQuery) ||
            (v.company_id && getCompany(v.company_id)?.title.toLowerCase().includes(lowerQuery))
        );
    }, [data, searchQuery, companyMap]);

    if (isLoading) {
        return (
            <div className="space-y-4 p-4">
                {[1, 2, 3].map((i) => (
                    <Card key={i} className="h-32">
                        <CardHeader className="p-4 pb-2 space-y-2">
                            <div className="flex justify-between">
                                <Skeleton className="h-3 w-20" />
                                <Skeleton className="h-4 w-8" />
                            </div>
                            <Skeleton className="h-4 w-full" />
                            <Skeleton className="h-4 w-2/3" />
                        </CardHeader>
                    </Card>
                ))}
            </div>
        );
    }

    if (error) {
        return (
            <div className="p-4">
                <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-4 text-center">
                    <p className="text-destructive font-medium">{t('common.error')}</p>
                </div>
            </div>
        );
    }

    return (
        <div className="p-4 space-y-4 pb-24">
            <div className="space-y-4 sticky top-0 bg-background/95 backdrop-blur z-30 pb-2 -mx-4 px-4 pt-2">
                <div className="space-y-1">
                    <h2 className="text-2xl font-bold">{t('dashboard.vacancies.title')}</h2>
                    <p className="text-sm text-muted-foreground">{t('dashboard.vacancies.active')}</p>
                </div>
                <div className="relative">
                    <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
                    <Input
                        placeholder={t('common.search') + "..."}
                        className="pl-9 bg-muted/50"
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                    />
                </div>
            </div>

            <div className="grid gap-3">
                {filteredVacancies.length === 0 ? (
                    <div className="text-center py-10 text-muted-foreground">
                        {t('dashboard.vacancies.no_vacancies')}
                    </div>
                ) : (
                    filteredVacancies.map((vacancy) => {
                        const company = getCompany(vacancy.company_id);
                        return (
                            <Card
                                key={vacancy.id}
                                className="cursor-pointer premium-hover flex flex-col group overflow-hidden border-muted-foreground/10"
                                onClick={() => setSelectedVacancy(vacancy)}
                            >
                                <CardHeader className="p-4 space-y-2">
                                    <div className="flex items-start justify-between gap-2">
                                        <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
                                            <Building2 className="h-3 w-3 shrink-0" />
                                            <span className="truncate max-w-[150px] font-medium">{company?.title || vacancy.direction}</span>
                                        </div>
                                        {vacancy.hot && (
                                            <Badge variant="destructive" className="h-4 px-1 gap-0.5 text-[9px] shrink-0 font-bold">
                                                <Flame className="h-2.5 w-2.5 fill-current" />
                                                Hot
                                            </Badge>
                                        )}
                                    </div>
                                    <div
                                        className="font-bold leading-tight line-clamp-2 text-base group-hover:text-primary transition-colors"
                                        dangerouslySetInnerHTML={{ __html: vacancy.title }}
                                    />
                                    <div className="flex items-center gap-2 opacity-70 text-xs text-muted-foreground justify-between pt-1">
                                        <div className="flex items-center gap-1.5">
                                            <MapPin className="h-3 w-3 shrink-0" />
                                            <span>{vacancy.city}</span>
                                        </div>
                                        <div className="flex items-center gap-1.5">
                                            <CalendarIconLucide className="h-3 w-3 shrink-0" />
                                            <span>{format(new Date(vacancy.created_at), 'dd MMM', { locale: dateLocale })}</span>
                                        </div>
                                    </div>
                                </CardHeader>
                            </Card>
                        );
                    })
                )}
            </div>

            <Dialog open={!!selectedVacancy} onOpenChange={(open) => !open && setSelectedVacancy(null)}>
                <DialogContent className="max-w-lg max-h-[85vh] overflow-y-auto w-[90%] rounded-lg flex flex-col p-0 gap-0">
                    {selectedVacancy && (
                        <>
                            <div className="p-6 pb-2">
                                <DialogHeader className="mb-2 space-y-3 text-left">
                                    <div>
                                        <div className="flex items-center gap-2 text-sm text-muted-foreground mb-1 flex-wrap">
                                            <Badge variant="outline" className="font-mono text-xs">#{selectedVacancy.id}</Badge>
                                            {selectedVacancy.company_id && (
                                                <>
                                                    <Badge variant="outline" className="font-medium">{getCompany(selectedVacancy.company_id)?.title}</Badge>
                                                </>
                                            )}
                                        </div>
                                        <DialogTitle
                                            className="text-xl mt-2 select-text"
                                            dangerouslySetInnerHTML={{ __html: selectedVacancy.title }}
                                        />
                                        <div className="flex items-center gap-4 mt-2 text-sm text-muted-foreground">
                                            <span className="flex items-center gap-1.5">
                                                <MapPin className="h-3 w-3" /> {selectedVacancy.city}
                                            </span>
                                            <span className="flex items-center gap-1.5">
                                                <CalendarIconLucide className="h-3 w-3" /> {format(new Date(selectedVacancy.created_at), 'PPP', { locale: dateLocale })}
                                            </span>
                                        </div>
                                    </div>
                                </DialogHeader>

                                <div className="prose prose-sm dark:prose-invert max-w-none select-text [&_*]:!text-foreground/90 mt-4">
                                    <div dangerouslySetInnerHTML={{ __html: selectedVacancy.content }} />
                                </div>
                            </div>

                            <div className="p-4 border-t sticky bottom-0 bg-background/95 backdrop-blur mt-auto">
                                <Button className="w-full h-11" onClick={() => {
                                    onSelectVacancy(selectedVacancy);
                                    setSelectedVacancy(null);
                                }}>
                                    {selectButtonText || t('registration.submit')}
                                </Button>
                            </div>
                        </>
                    )}
                </DialogContent>
            </Dialog>
        </div>
    )
}
