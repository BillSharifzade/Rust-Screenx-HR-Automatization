'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { motion } from 'framer-motion';
import {
    Users as UsersIcon,
    UserPlus,
    Pencil,
    Trash2,
    KeyRound,
    Loader2,
    ShieldAlert,
    Mail,
    Clock,
} from 'lucide-react';
import {
    AuthUser,
    listUsers,
    createUser,
    updateUser,
    deleteUser,
    resetUserPassword,
} from '@/lib/auth';
import { useAuth } from '@/lib/auth-context';
import { useTranslation } from '@/lib/i18n-context';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';
import { Card, CardContent } from '@/components/ui/card';
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from '@/components/ui/select';
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from '@/components/ui/dialog';
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
} from '@/components/ui/alert-dialog';
import { toast } from 'sonner';

const ROLES = ['admin', 'hr', 'manager'] as const;

function roleBadgeVariant(role: string): 'default' | 'secondary' | 'outline' {
    if (role === 'admin') return 'default';
    if (role === 'hr') return 'secondary';
    return 'outline';
}

function errorToMessage(err: any, t: (k: string) => string): string {
    const code = err?.message || '';
    if (code.includes('email_exists')) return t('auth.errors.email_exists');
    if (code.includes('cannot_remove_last_admin')) return t('auth.errors.cannot_remove_last_admin');
    if (code.includes('password_too_short')) return t('auth.errors.password_too_short');
    if (code.includes('invalid_email')) return t('auth.errors.invalid_email');
    return t('auth.errors.generic');
}

export default function UsersPage() {
    const { t } = useTranslation();
    const { user: me, isAdmin, loading: authLoading } = useAuth();
    const router = useRouter();
    const qc = useQueryClient();

    // Non-admins should not see this page.
    useEffect(() => {
        if (!authLoading && !isAdmin) {
            router.replace('/dashboard');
        }
    }, [authLoading, isAdmin, router]);

    const { data: users, isLoading } = useQuery({
        queryKey: ['admin-users'],
        queryFn: listUsers,
        enabled: isAdmin,
    });

    // ---- Create / edit dialog ----
    const [formOpen, setFormOpen] = useState(false);
    const [editing, setEditing] = useState<AuthUser | null>(null);
    const [name, setName] = useState('');
    const [email, setEmail] = useState('');
    const [password, setPassword] = useState('');
    const [role, setRole] = useState<string>('hr');
    const [isActive, setIsActive] = useState(true);
    const [mustChange, setMustChange] = useState(true);

    const openCreate = () => {
        setEditing(null);
        setName('');
        setEmail('');
        setPassword('');
        setRole('hr');
        setIsActive(true);
        setMustChange(true);
        setFormOpen(true);
    };

    const openEdit = (u: AuthUser) => {
        setEditing(u);
        setName(u.name);
        setEmail(u.email);
        setRole(u.role);
        setIsActive(u.is_active);
        setFormOpen(true);
    };

    const saveMutation = useMutation({
        mutationFn: async () => {
            if (editing) {
                return updateUser(editing.id, { name, email, role, is_active: isActive });
            }
            return createUser({
                name,
                email,
                password,
                role,
                is_active: isActive,
                must_change_password: mustChange,
            });
        },
        onSuccess: () => {
            qc.invalidateQueries({ queryKey: ['admin-users'] });
            toast.success(editing ? t('auth.users.updated') : t('auth.users.created'));
            setFormOpen(false);
        },
        onError: (err) => toast.error(errorToMessage(err, t)),
    });

    // ---- Reset password dialog ----
    const [pwOpen, setPwOpen] = useState(false);
    const [pwTarget, setPwTarget] = useState<AuthUser | null>(null);
    const [newPw, setNewPw] = useState('');
    const [pwMustChange, setPwMustChange] = useState(true);

    const openReset = (u: AuthUser) => {
        setPwTarget(u);
        setNewPw('');
        setPwMustChange(true);
        setPwOpen(true);
    };

    const resetMutation = useMutation({
        mutationFn: async () => resetUserPassword(pwTarget!.id, newPw, pwMustChange),
        onSuccess: () => {
            qc.invalidateQueries({ queryKey: ['admin-users'] });
            toast.success(t('auth.users.password_reset'));
            setPwOpen(false);
        },
        onError: (err) => toast.error(errorToMessage(err, t)),
    });

    const deleteMutation = useMutation({
        mutationFn: async (id: string) => deleteUser(id),
        onSuccess: () => {
            qc.invalidateQueries({ queryKey: ['admin-users'] });
            toast.success(t('auth.users.deleted'));
        },
        onError: (err) => toast.error(errorToMessage(err, t)),
    });

    const submitForm = (e: React.FormEvent) => {
        e.preventDefault();
        if (!name.trim()) return toast.error(t('auth.errors.name_required'));
        if (!editing && password.length < 8) return toast.error(t('auth.errors.password_too_short'));
        saveMutation.mutate();
    };

    if (!isAdmin) {
        return (
            <div className="flex flex-1 flex-col items-center justify-center gap-3 text-center text-muted-foreground">
                <ShieldAlert className="h-10 w-10" />
                <p>{t('auth.users.forbidden')}</p>
            </div>
        );
    }

    return (
        <div className="space-y-6">
            <div className="flex items-center justify-between gap-4">
                <div className="flex items-center gap-3">
                    <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
                        <UsersIcon className="h-5 w-5 text-primary" />
                    </div>
                    <div>
                        <h1 className="text-4xl font-bold tracking-tight bg-gradient-to-r from-primary to-primary/60 bg-clip-text text-transparent">{t('auth.users.title')}</h1>
                        <p className="text-sm text-muted-foreground">{t('auth.users.subtitle')}</p>
                    </div>
                </div>
                <Button onClick={openCreate}>
                    <UserPlus className="mr-2 h-4 w-4" />
                    {t('auth.users.add')}
                </Button>
            </div>

            {isLoading ? (
                <div className="flex items-center justify-center py-20">
                    <Loader2 className="h-6 w-6 animate-spin text-primary" />
                </div>
            ) : (
                <div className="grid gap-3">
                    {(users ?? []).map((u, i) => (
                        <motion.div
                            key={u.id}
                            initial={{ opacity: 0, y: 8 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ duration: 0.2, delay: i * 0.03 }}
                        >
                            <Card className="transition-colors hover:border-primary/30">
                                <CardContent className="flex flex-col gap-3 p-4 sm:flex-row sm:items-center sm:justify-between">
                                    <div className="min-w-0 space-y-1">
                                        <div className="flex flex-wrap items-center gap-2">
                                            <span className="font-semibold">{u.name}</span>
                                            <Badge variant={roleBadgeVariant(u.role)}>
                                                {t(`auth.roles.${u.role}`)}
                                            </Badge>
                                            {!u.is_active && (
                                                <Badge variant="outline" className="text-destructive">
                                                    {t('auth.users.inactive')}
                                                </Badge>
                                            )}
                                            {u.id === me?.id && (
                                                <Badge variant="outline">{t('auth.users.you')}</Badge>
                                            )}
                                        </div>
                                        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
                                            <span className="flex items-center gap-1">
                                                <Mail className="h-3 w-3" /> {u.email}
                                            </span>
                                            {u.last_login_at && (
                                                <span className="flex items-center gap-1">
                                                    <Clock className="h-3 w-3" />
                                                    {t('auth.users.last_login')}:{' '}
                                                    {new Date(u.last_login_at).toLocaleString()}
                                                </span>
                                            )}
                                        </div>
                                    </div>
                                    <div className="flex shrink-0 items-center gap-1">
                                        <Button variant="ghost" size="icon" onClick={() => openEdit(u)} title={t('common.edit')}>
                                            <Pencil className="h-4 w-4" />
                                        </Button>
                                        <Button variant="ghost" size="icon" onClick={() => openReset(u)} title={t('auth.users.reset_password')}>
                                            <KeyRound className="h-4 w-4" />
                                        </Button>
                                        <AlertDialog>
                                            <AlertDialogTrigger asChild>
                                                <Button
                                                    variant="ghost"
                                                    size="icon"
                                                    className="text-destructive hover:text-destructive"
                                                    disabled={u.id === me?.id}
                                                    title={t('common.delete')}
                                                >
                                                    <Trash2 className="h-4 w-4" />
                                                </Button>
                                            </AlertDialogTrigger>
                                            <AlertDialogContent>
                                                <AlertDialogHeader>
                                                    <AlertDialogTitle>{t('auth.users.delete_confirm')}</AlertDialogTitle>
                                                    <AlertDialogDescription>
                                                        {t('auth.users.delete_desc')} <strong>{u.name}</strong>
                                                    </AlertDialogDescription>
                                                </AlertDialogHeader>
                                                <AlertDialogFooter>
                                                    <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
                                                    <AlertDialogAction
                                                        className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                                                        onClick={() => deleteMutation.mutate(u.id)}
                                                    >
                                                        {t('common.delete')}
                                                    </AlertDialogAction>
                                                </AlertDialogFooter>
                                            </AlertDialogContent>
                                        </AlertDialog>
                                    </div>
                                </CardContent>
                            </Card>
                        </motion.div>
                    ))}
                    {users && users.length === 0 && (
                        <p className="py-10 text-center text-sm text-muted-foreground">
                            {t('auth.users.empty')}
                        </p>
                    )}
                </div>
            )}

            {/* Create / edit dialog */}
            <Dialog open={formOpen} onOpenChange={setFormOpen}>
                <DialogContent className="sm:max-w-md">
                    <form onSubmit={submitForm}>
                        <DialogHeader>
                            <DialogTitle>
                                {editing ? t('auth.users.edit_title') : t('auth.users.add')}
                            </DialogTitle>
                            <DialogDescription>
                                {editing ? t('auth.users.edit_desc') : t('auth.users.add_desc')}
                            </DialogDescription>
                        </DialogHeader>
                        <div className="space-y-4 py-4">
                            <div className="space-y-2">
                                <Label htmlFor="u-name">{t('auth.users.name')}</Label>
                                <Input id="u-name" value={name} onChange={(e) => setName(e.target.value)} required />
                            </div>
                            <div className="space-y-2">
                                <Label htmlFor="u-email">{t('auth.login.email')}</Label>
                                <Input id="u-email" type="email" value={email} onChange={(e) => setEmail(e.target.value)} required />
                            </div>
                            {!editing && (
                                <div className="space-y-2">
                                    <Label htmlFor="u-pw">{t('auth.login.password')}</Label>
                                    <Input
                                        id="u-pw"
                                        type="password"
                                        autoComplete="new-password"
                                        value={password}
                                        onChange={(e) => setPassword(e.target.value)}
                                        placeholder={t('auth.users.password_hint')}
                                        required
                                    />
                                </div>
                            )}
                            <div className="space-y-2">
                                <Label>{t('auth.users.role')}</Label>
                                <Select value={role} onValueChange={setRole}>
                                    <SelectTrigger>
                                        <SelectValue />
                                    </SelectTrigger>
                                    <SelectContent>
                                        {ROLES.map((r) => (
                                            <SelectItem key={r} value={r}>
                                                {t(`auth.roles.${r}`)}
                                            </SelectItem>
                                        ))}
                                    </SelectContent>
                                </Select>
                            </div>
                            <div className="flex items-center justify-between rounded-lg border p-3">
                                <Label htmlFor="u-active" className="cursor-pointer">
                                    {t('auth.users.active')}
                                </Label>
                                <Switch id="u-active" checked={isActive} onCheckedChange={setIsActive} />
                            </div>
                            {!editing && (
                                <div className="flex items-center justify-between rounded-lg border p-3">
                                    <Label htmlFor="u-mc" className="cursor-pointer">
                                        {t('auth.users.must_change')}
                                    </Label>
                                    <Switch id="u-mc" checked={mustChange} onCheckedChange={setMustChange} />
                                </div>
                            )}
                        </div>
                        <DialogFooter>
                            <Button type="button" variant="outline" onClick={() => setFormOpen(false)}>
                                {t('common.cancel')}
                            </Button>
                            <Button type="submit" disabled={saveMutation.isPending}>
                                {saveMutation.isPending ? (
                                    <Loader2 className="h-4 w-4 animate-spin" />
                                ) : (
                                    t('common.save')
                                )}
                            </Button>
                        </DialogFooter>
                    </form>
                </DialogContent>
            </Dialog>

            {/* Reset password dialog */}
            <Dialog open={pwOpen} onOpenChange={setPwOpen}>
                <DialogContent className="sm:max-w-md">
                    <DialogHeader>
                        <DialogTitle>{t('auth.users.reset_password')}</DialogTitle>
                        <DialogDescription>
                            {pwTarget?.name} — {pwTarget?.email}
                        </DialogDescription>
                    </DialogHeader>
                    <div className="space-y-4 py-4">
                        <div className="space-y-2">
                            <Label htmlFor="r-pw">{t('auth.change.new')}</Label>
                            <Input
                                id="r-pw"
                                type="password"
                                autoComplete="new-password"
                                value={newPw}
                                onChange={(e) => setNewPw(e.target.value)}
                            />
                        </div>
                        <div className="flex items-center justify-between rounded-lg border p-3">
                            <Label htmlFor="r-mc" className="cursor-pointer">
                                {t('auth.users.must_change')}
                            </Label>
                            <Switch id="r-mc" checked={pwMustChange} onCheckedChange={setPwMustChange} />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button type="button" variant="outline" onClick={() => setPwOpen(false)}>
                            {t('common.cancel')}
                        </Button>
                        <Button
                            onClick={() => {
                                if (newPw.length < 8) return toast.error(t('auth.errors.password_too_short'));
                                resetMutation.mutate();
                            }}
                            disabled={resetMutation.isPending}
                        >
                            {resetMutation.isPending ? (
                                <Loader2 className="h-4 w-4 animate-spin" />
                            ) : (
                                t('auth.users.reset_password')
                            )}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </div>
    );
}
