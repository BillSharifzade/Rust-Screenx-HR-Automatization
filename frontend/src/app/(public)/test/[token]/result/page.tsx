'use client';

import { useParams, useRouter } from 'next/navigation';
import { useQuery } from '@tanstack/react-query';
import { apiFetch } from '@/lib/api';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { CheckCircle2, Home, Loader2, AlertCircle } from 'lucide-react';
import { useTranslation } from '@/lib/i18n-context';

export default function TestResultPage() {
    const { t } = useTranslation();
    const params = useParams();
    const router = useRouter();
    const token = params.token as string;

    const { data: statusData, isLoading, error } = useQuery({
        queryKey: ['test-status', token],
        queryFn: () => apiFetch<any>(`/api/public/tests/${token}/status`),
        refetchInterval: false,
    });

    if (isLoading) {
        return (
            <div className="flex h-screen items-center justify-center bg-muted/20">
                <Loader2 className="h-8 w-8 animate-spin text-primary" />
            </div>
        );
    }

    if (error || !statusData) {
        return (
            <div className="flex h-screen items-center justify-center bg-muted/20 p-4">
                <Card className="max-w-md w-full">
                    <CardHeader>
                        <CardTitle className="flex items-center gap-2 text-destructive">
                            <AlertCircle className="h-5 w-5" />
                            {t('common.error')}
                        </CardTitle>
                    </CardHeader>
                    <CardContent>
                        <p>{t('test.error_load')}</p>
                        <Button className="mt-4 w-full" onClick={() => router.push('/')}>
                            {t('test.return_home')}
                        </Button>
                    </CardContent>
                </Card>
            </div>
        );
    }

    const { status } = statusData;

    return (
        <div className="min-h-screen bg-muted/20 flex items-center justify-center p-4">
            <Card className="max-w-md w-full shadow-lg border-primary/10">
                <CardHeader className="text-center pb-2">
                    <div className="mx-auto mb-4 h-16 w-16 rounded-full bg-green-50 dark:bg-green-900/20 flex items-center justify-center">
                        <CheckCircle2 className="h-10 w-10 text-green-600" />
                    </div>
                    <CardTitle className="text-2xl font-bold">
                        {status === 'completed' ? t('test.submit_success_title') || "Test Completed" : t('test.finalized_title') || "Test Processed"}
                    </CardTitle>
                </CardHeader>
                <CardContent className="text-center space-y-6 pt-4">
                    <p className="text-muted-foreground">
                        {status === 'completed'
                            ? t('test.submit_success_desc') || "Thank you for completing the test. Your results have been submitted and our team will review them shortly."
                            : t('test.already_submitted') || "This test attempt has already been finalized."}
                    </p>
                    <div className="pt-4">
                        <Button variant="outline" className="w-full gap-2" onClick={() => (window.location.href = '/')}>
                            <Home className="h-4 w-4" />
                            {t('test.return_home')}
                        </Button>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
