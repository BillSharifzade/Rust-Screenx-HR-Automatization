'use client';

import { useQuery } from '@tanstack/react-query';
import { apiFetch } from '@/lib/api';
import { VacancyPublicListResponse } from '@/types/api';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Building2, MapPin, DollarSign, ArrowRight } from 'lucide-react';
import Link from 'next/link';
import { format } from 'date-fns';

export default function PublicVacanciesPage() {
    const { data, isLoading, error } = useQuery({
        queryKey: ['public-vacancies'],
        queryFn: () => apiFetch<VacancyPublicListResponse>('/api/public/vacancies'),
    });

    if (isLoading) {
        return <div className="container mx-auto py-10">Loading opportunities...</div>;
    }
    return (
        <div className="container mx-auto py-12 px-4 max-w-5xl">
            <div className="text-center mb-12 space-y-4">
                <h1 className="text-4xl font-bold tracking-tight lg:text-5xl">
                    Открытые вакансии
                </h1>
                <p className="text-xl text-muted-foreground max-w-2xl mx-auto">
                    Присоединяйтесь к нашей команде и помогите нам строить будущее.
                    Мы ищем талантливых людей, увлеченных своим делом.
                </p>
            </div>

            {isLoading ? (
                <div className="flex justify-center py-12">
                    <div className="text-muted-foreground">Загрузка вакансий...</div>
                </div>
            ) : error ? (
                <div className="text-center text-destructive py-12">
                    Не удалось загрузить вакансии. Пожалуйста, попробуйте позже.
                </div>
            ) : (
                <div className="grid gap-6">
                    {data?.items.map((vacancy: any) => (
                        <Card key={vacancy.id} className="transition-all hover:shadow-lg border-l-4 border-l-primary/50">
                            <CardHeader className="flex flex-col md:flex-row md:items-start md:justify-between gap-4">
                                <div className="space-y-2">
                                    <CardTitle className="text-2xl font-bold text-primary">
                                        {vacancy.title}
                                    </CardTitle>
                                    <div className="flex flex-wrap items-center gap-4 text-muted-foreground">
                                        <div className="flex items-center gap-1.5">
                                            <Building2 className="h-4 w-4" />
                                            {vacancy.company}
                                        </div>
                                        <div className="flex items-center gap-1.5">
                                            <MapPin className="h-4 w-4" />
                                            {vacancy.location}
                                        </div>
                                        {(vacancy.salary_from || vacancy.salary_to) && (
                                            <div className="flex items-center gap-1.5 text-green-600 dark:text-green-400 font-medium">
                                                <DollarSign className="h-4 w-4" />
                                                {vacancy.salary_from} - {vacancy.salary_to} {vacancy.currency}
                                            </div>
                                        )}
                                    </div>
                                </div>
                                <div className="flex flex-col gap-2 min-w-[120px]">
                                    <Badge variant="secondary" className="justify-center py-1">
                                        {vacancy.employment_type || 'Полная занятость'}
                                    </Badge>
                                    <Button asChild className="w-full group">
                                        <Link href={`/vacancies/${vacancy.id}`}>
                                            Откликнуться
                                            <ArrowRight className="ml-2 h-4 w-4 transition-transform group-hover:translate-x-1" />
                                        </Link>
                                    </Button>
                                </div>
                            </CardHeader>
                            <CardContent>
                                <p className="text-muted-foreground line-clamp-3 mb-4">
                                    {vacancy.description || 'Описание вакансии недоступно.'}
                                </p>
                                <div className="text-xs text-muted-foreground/60">
                                    Опубликовано: {format(new Date(vacancy.published_at || new Date()), 'PPP')}
                                </div>
                            </CardContent>
                        </Card>
                    ))}

                    {data?.items.length === 0 && (
                        <div className="text-center py-12 text-muted-foreground">
                            В данный момент открытых вакансий нет. Пожалуйста, заходите позже.
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}
