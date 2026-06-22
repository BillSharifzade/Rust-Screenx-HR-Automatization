'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { motion } from 'framer-motion';
import { Loader2, KeyRound, ShieldCheck } from 'lucide-react';
import { useAuth } from '@/lib/auth-context';
import { changeMyPassword } from '@/lib/auth';
import { useTranslation } from '@/lib/i18n-context';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { toast } from 'sonner';

function FullscreenSpinner() {
    return (
        <div className="flex min-h-screen items-center justify-center bg-background">
            <Loader2 className="h-8 w-8 animate-spin text-primary" />
        </div>
    );
}

/** Forced password rotation shown when `must_change_password` is set. */
function ForceChangePassword() {
    const { t } = useTranslation();
    const { refresh, logout } = useAuth();
    const [current, setCurrent] = useState('');
    const [next, setNext] = useState('');
    const [confirm, setConfirm] = useState('');
    const [loading, setLoading] = useState(false);

    const submit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (next.length < 8) {
            toast.error(t('auth.errors.password_too_short'));
            return;
        }
        if (next !== confirm) {
            toast.error(t('auth.errors.password_mismatch'));
            return;
        }
        setLoading(true);
        try {
            await changeMyPassword(current, next);
            toast.success(t('auth.change.success'));
            await refresh();
        } catch (err: any) {
            const code = err?.message || '';
            toast.error(
                code.includes('invalid_current_password')
                    ? t('auth.errors.invalid_current_password')
                    : t('auth.errors.generic'),
            );
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="flex min-h-screen items-center justify-center bg-gradient-to-br from-background via-background to-primary/5 p-4">
            <motion.div
                initial={{ opacity: 0, y: 12 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3 }}
                className="w-full max-w-md"
            >
                <Card className="border-primary/10 shadow-xl">
                    <CardHeader className="space-y-3 text-center">
                        <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-primary/10">
                            <ShieldCheck className="h-6 w-6 text-primary" />
                        </div>
                        <CardTitle className="text-2xl">{t('auth.change.title')}</CardTitle>
                        <CardDescription>{t('auth.change.subtitle')}</CardDescription>
                    </CardHeader>
                    <CardContent>
                        <form onSubmit={submit} className="space-y-4">
                            <div className="space-y-2">
                                <Label htmlFor="cur">{t('auth.change.current')}</Label>
                                <Input
                                    id="cur"
                                    type="password"
                                    value={current}
                                    onChange={(e) => setCurrent(e.target.value)}
                                    required
                                />
                            </div>
                            <div className="space-y-2">
                                <Label htmlFor="new">{t('auth.change.new')}</Label>
                                <Input
                                    id="new"
                                    type="password"
                                    value={next}
                                    onChange={(e) => setNext(e.target.value)}
                                    required
                                />
                            </div>
                            <div className="space-y-2">
                                <Label htmlFor="cfm">{t('auth.change.confirm')}</Label>
                                <Input
                                    id="cfm"
                                    type="password"
                                    value={confirm}
                                    onChange={(e) => setConfirm(e.target.value)}
                                    required
                                />
                            </div>
                            <Button type="submit" className="w-full" disabled={loading}>
                                {loading ? (
                                    <Loader2 className="h-4 w-4 animate-spin" />
                                ) : (
                                    <>
                                        <KeyRound className="mr-2 h-4 w-4" />
                                        {t('auth.change.submit')}
                                    </>
                                )}
                            </Button>
                            <button
                                type="button"
                                onClick={logout}
                                className="w-full text-center text-xs text-muted-foreground hover:text-primary transition-colors"
                            >
                                {t('auth.logout')}
                            </button>
                        </form>
                    </CardContent>
                </Card>
            </motion.div>
        </div>
    );
}

export function AuthGuard({ children }: { children: React.ReactNode }) {
    const { user, loading } = useAuth();
    const router = useRouter();

    useEffect(() => {
        if (!loading && !user) {
            router.replace('/login');
        }
    }, [loading, user, router]);

    if (loading || !user) {
        return <FullscreenSpinner />;
    }

    if (user.must_change_password) {
        return <ForceChangePassword />;
    }

    return <>{children}</>;
}
