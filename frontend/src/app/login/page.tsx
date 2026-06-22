'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { motion } from 'framer-motion';
import { Loader2, LogIn, Lock, Mail, KeyRound, ShieldCheck } from 'lucide-react';
import { login, changeMyPassword } from '@/lib/auth';
import { tokenStore } from '@/lib/store';
import { useTranslation } from '@/lib/i18n-context';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { ModeToggle } from '@/components/mode-toggle';
import { LanguageToggle } from '@/components/language-toggle';
import { toast } from 'sonner';

export default function LoginPage() {
    const { t } = useTranslation();
    const router = useRouter();

    const [email, setEmail] = useState('');
    const [password, setPassword] = useState('');
    const [loading, setLoading] = useState(false);

    // Forced password-rotation step (seeded admin / admin-reset accounts).
    const [mustChange, setMustChange] = useState(false);
    const [newPassword, setNewPassword] = useState('');
    const [confirmPassword, setConfirmPassword] = useState('');

    // Already authenticated -> skip the login screen.
    useEffect(() => {
        if (tokenStore.getToken()) {
            router.replace('/dashboard');
        }
    }, [router]);

    const onLogin = async (e: React.FormEvent) => {
        e.preventDefault();
        setLoading(true);
        try {
            const res = await login(email.trim(), password);
            if (res.must_change_password) {
                setMustChange(true);
                toast.message(t('auth.change.required'));
            } else {
                toast.success(t('auth.login.success'));
                router.replace('/dashboard');
            }
        } catch (err: any) {
            const code = err?.message || '';
            if (code.includes('too_many_attempts')) {
                toast.error(t('auth.errors.too_many_attempts'));
            } else if (code.includes('invalid_credentials')) {
                toast.error(t('auth.errors.invalid_credentials'));
            } else {
                toast.error(t('auth.errors.generic'));
            }
        } finally {
            setLoading(false);
        }
    };

    const onChange = async (e: React.FormEvent) => {
        e.preventDefault();
        if (newPassword.length < 8) {
            toast.error(t('auth.errors.password_too_short'));
            return;
        }
        if (newPassword !== confirmPassword) {
            toast.error(t('auth.errors.password_mismatch'));
            return;
        }
        setLoading(true);
        try {
            // The password just used to log in is the current one.
            await changeMyPassword(password, newPassword);
            toast.success(t('auth.change.success'));
            router.replace('/dashboard');
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
        <div className="relative flex min-h-screen items-center justify-center bg-gradient-to-br from-background via-background to-primary/5 p-4">
            {/* Decorative blobs */}
            <div className="pointer-events-none absolute inset-0 overflow-hidden">
                <div className="absolute -top-24 -left-24 h-72 w-72 rounded-full bg-primary/10 blur-3xl" />
                <div className="absolute -bottom-24 -right-24 h-72 w-72 rounded-full bg-primary/5 blur-3xl" />
            </div>

            <div className="absolute top-4 right-4 z-10 flex items-center gap-1 rounded-full border border-primary/10 bg-background/80 p-1.5 shadow-sm backdrop-blur-sm">
                <LanguageToggle variant="inline" />
                <div className="mx-1 h-4 w-px bg-border" />
                <ModeToggle />
            </div>

            <motion.div
                initial={{ opacity: 0, y: 16 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.35 }}
                className="relative z-10 w-full max-w-md"
            >
                <div className="mb-6 flex flex-col items-center gap-3 text-center">
                    <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-primary text-2xl font-bold text-primary-foreground shadow-lg shadow-primary/20">
                        K
                    </div>
                    <div>
                        <h1 className="text-2xl font-bold tracking-tight">KoinotHR</h1>
                        <p className="text-sm text-muted-foreground">{t('auth.login.tagline')}</p>
                    </div>
                </div>

                <Card className="border-primary/10 shadow-xl">
                    {!mustChange ? (
                        <>
                            <CardHeader className="space-y-1">
                                <CardTitle className="text-xl">{t('auth.login.title')}</CardTitle>
                                <CardDescription>{t('auth.login.subtitle')}</CardDescription>
                            </CardHeader>
                            <CardContent>
                                <form onSubmit={onLogin} className="space-y-4">
                                    <div className="space-y-2">
                                        <Label htmlFor="email">{t('auth.login.email')}</Label>
                                        <div className="relative">
                                            <Mail className="absolute left-3 top-3 h-4 w-4 text-muted-foreground" />
                                            <Input
                                                id="email"
                                                type="email"
                                                autoComplete="username"
                                                placeholder="admin@koinot.local"
                                                className="pl-9"
                                                value={email}
                                                onChange={(e) => setEmail(e.target.value)}
                                                required
                                            />
                                        </div>
                                    </div>
                                    <div className="space-y-2">
                                        <Label htmlFor="password">{t('auth.login.password')}</Label>
                                        <div className="relative">
                                            <Lock className="absolute left-3 top-3 h-4 w-4 text-muted-foreground" />
                                            <Input
                                                id="password"
                                                type="password"
                                                autoComplete="current-password"
                                                placeholder="••••••••"
                                                className="pl-9"
                                                value={password}
                                                onChange={(e) => setPassword(e.target.value)}
                                                required
                                            />
                                        </div>
                                    </div>
                                    <Button type="submit" className="w-full" disabled={loading}>
                                        {loading ? (
                                            <Loader2 className="h-4 w-4 animate-spin" />
                                        ) : (
                                            <>
                                                <LogIn className="mr-2 h-4 w-4" />
                                                {t('auth.login.submit')}
                                            </>
                                        )}
                                    </Button>
                                </form>
                            </CardContent>
                        </>
                    ) : (
                        <>
                            <CardHeader className="space-y-3 text-center">
                                <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-primary/10">
                                    <ShieldCheck className="h-6 w-6 text-primary" />
                                </div>
                                <CardTitle className="text-xl">{t('auth.change.title')}</CardTitle>
                                <CardDescription>{t('auth.change.subtitle')}</CardDescription>
                            </CardHeader>
                            <CardContent>
                                <form onSubmit={onChange} className="space-y-4">
                                    <div className="space-y-2">
                                        <Label htmlFor="new">{t('auth.change.new')}</Label>
                                        <Input
                                            id="new"
                                            type="password"
                                            autoComplete="new-password"
                                            value={newPassword}
                                            onChange={(e) => setNewPassword(e.target.value)}
                                            required
                                        />
                                    </div>
                                    <div className="space-y-2">
                                        <Label htmlFor="confirm">{t('auth.change.confirm')}</Label>
                                        <Input
                                            id="confirm"
                                            type="password"
                                            autoComplete="new-password"
                                            value={confirmPassword}
                                            onChange={(e) => setConfirmPassword(e.target.value)}
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
                                </form>
                            </CardContent>
                        </>
                    )}
                </Card>
            </motion.div>
        </div>
    );
}
