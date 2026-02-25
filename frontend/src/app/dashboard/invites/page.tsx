'use client';

import { useState, useMemo, useEffect, useRef, Suspense } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useSearchParams } from 'next/navigation';
import { apiFetch } from '@/lib/api';
import { Test, CreateInvitePayload } from '@/types/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Select, SelectContent, SelectGroup, SelectItem, SelectLabel, SelectTrigger, SelectValue, SelectSeparator } from '@/components/ui/select';
import { Badge } from '@/components/ui/badge';
import { toast } from 'sonner';
import { User, Mail, AtSign, X, Send, Clock, CheckCircle2, XCircle, FileText, Users, Trash2, Loader2, AlertCircle, MonitorPlay, ClipboardCheck } from 'lucide-react';
import { useTranslation } from '@/lib/i18n-context';
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

interface Candidate {
    id: string;
    name: string;
    email: string;
    phone?: string;
    telegram_id?: number;
}

interface TestInvite {
    id: string;
    test_id: string;
    candidate_email: string;
    candidate_name: string;
    status: string;
    created_at: string;
    expires_at: string;
}

function InvitesPageContent() {
    const { t } = useTranslation();
    const queryClient = useQueryClient();
    const searchParams = useSearchParams();
    const [selectedTest, setSelectedTest] = useState<string>('');
    const [email, setEmail] = useState('');
    const [name, setName] = useState('');
    const [searchQuery, setSearchQuery] = useState('');
    const [selectedCandidate, setSelectedCandidate] = useState<Candidate | null>(null);
    const [showDropdown, setShowDropdown] = useState(false);
    const dropdownRef = useRef<HTMLDivElement>(null);

    const testIdFromUrl = searchParams.get('test');

    const { data: testsList, isLoading: testsLoading } = useQuery({
        queryKey: ['tests'],
        queryFn: () => apiFetch<{ items: Test[] }>('/api/integration/tests?per_page=100'),
    });

    const { data: specificTest } = useQuery({
        queryKey: ['test', testIdFromUrl],
        queryFn: () => apiFetch<Test>(`/api/integration/tests/${testIdFromUrl}`),
        enabled: !!testIdFromUrl,
    });

    const allTests = useMemo(() => {
        const list = testsList?.items || [];
        if (specificTest && specificTest.id && !list.find(t => t.id === specificTest.id)) {
            return [specificTest, ...list];
        }
        return list;
    }, [testsList, specificTest]);

    useEffect(() => {
        if (testIdFromUrl) {
            setSelectedTest(testIdFromUrl);
        }
    }, [testIdFromUrl]);

    // Ensure selection is valid and persists when data loads
    useEffect(() => {
        if (testIdFromUrl && allTests.length > 0) {
            if (allTests.some((t: Test) => t.id === testIdFromUrl)) {
                setSelectedTest(testIdFromUrl);
            }
        }
    }, [allTests, testIdFromUrl]);

    const { data: candidates } = useQuery({
        queryKey: ['candidates'],
        queryFn: () => apiFetch<Candidate[]>('/api/integration/candidates'),
    });

    const { data: invites, isLoading: invitesLoading } = useQuery({
        queryKey: ['test-invites'],
        queryFn: () => apiFetch<{ items: TestInvite[] }>('/api/integration/test-invites'),
    });

    // Load candidate from URL params if present
    useEffect(() => {
        const candidateId = searchParams.get('candidate');
        if (candidateId && candidates) {
            const candidate = candidates.find((c: Candidate) => c.id === candidateId);
            if (candidate) {
                selectCandidate(candidate);
            }
        }
    }, [searchParams, candidates]);

    // Click outside to close dropdown
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
                setShowDropdown(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    // Robust search - matches name, email, phone, and telegram ID
    const filteredCandidates = useMemo(() => {
        if (!candidates || !searchQuery.trim()) return [];

        const query = searchQuery.toLowerCase().trim();
        const queryNoSpaces = query.replace(/\s+/g, "");

        return candidates.filter((candidate: Candidate) => {
            const candidateName = (candidate.name || "").toLowerCase();
            const candidateEmail = (candidate.email || "").toLowerCase();
            const candidatePhone = (candidate.phone || "").toLowerCase().replace(/\s+/g, "");
            const telegramId = String(candidate.telegram_id || "");

            return (
                candidateName.includes(query) ||
                candidateEmail.includes(query) ||
                candidatePhone.includes(queryNoSpaces) ||
                telegramId.includes(query)
            );
        }).slice(0, 5); // Limit to 5 results
    }, [candidates, searchQuery]);

    const selectCandidate = (candidate: Candidate) => {
        setSelectedCandidate(candidate);
        setName(candidate.name);
        setEmail(candidate.email);
        setSearchQuery('');
        setShowDropdown(false);
    };

    const clearSelectedCandidate = () => {
        setSelectedCandidate(null);
        setName('');
        setEmail('');
        setSearchQuery('');
    };

    const inviteMutation = useMutation({
        mutationFn: (data: CreateInvitePayload) =>
            apiFetch('/api/integration/test-invites', {
                method: 'POST',
                body: JSON.stringify(data),
            }),
        onSuccess: () => {
            toast.success(t('dashboard.invites.toasts.success'));
            clearSelectedCandidate();
            // Don't clear test selection if we're in "test specific mode", maybe? 
            // Better to clear it to avoid spamming the same test unless desired.
            // But if they came from "Send" button, they might want to send multiple.
            // Let's keep existing behavior: clear it.
            if (!searchParams.get('test')) {
                setSelectedTest('');
            }
            queryClient.invalidateQueries({ queryKey: ['test-invites'] });
        },
        onError: (error) => {
            const pendingMsg = "Candidate already has a pending test invitation";
            if (error.message && error.message.includes(pendingMsg)) {
                toast.error(t('dashboard.invites.toasts.candidate_has_pending'));
            } else {
                toast.error(`${t('dashboard.invites.toasts.error')}: ${error.message}`);
            }
        },
    });

    const deleteMutation = useMutation({
        mutationFn: (id: string) => apiFetch(`/api/integration/test-attempts/${id}`, { method: 'DELETE' }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ['test-invites'] });
            queryClient.invalidateQueries({ queryKey: ['test-attempts'] });
            toast.success(t('dashboard.attempts.delete_success'));
        },
        onError: (err: any) => {
            toast.error(err.message || t('common.error'));
        }
    });

    const handleDelete = (id: string) => {
        deleteMutation.mutate(id);
    };

    const handleInvite = (e: React.FormEvent) => {
        e.preventDefault();
        if (!selectedTest) {
            toast.error(t('dashboard.invites.toasts.select_test'));
            return;
        }
        if (!selectedCandidate) return;

        inviteMutation.mutate({
            test_id: selectedTest,
            candidate: {
                email: selectedCandidate.email,
                name: selectedCandidate.name,
                phone: selectedCandidate.phone,
                telegram_id: selectedCandidate.telegram_id,
                external_id: selectedCandidate.id,
            },
            expires_in_hours: 48,
        });
    };

    const getStatusBadge = (status: string) => {
        switch (status) {
            case 'pending':
                return <Badge variant="secondary" className="gap-1"><Clock className="h-3 w-3" /> {t('dashboard.invites.statuses.pending')}</Badge>;
            case 'in_progress':
                return <Badge variant="outline" className="gap-1 border-blue-500 text-blue-600 dark:text-blue-400"><Loader2 className="h-3 w-3 animate-spin" /> {t('dashboard.invites.statuses.in_progress')}</Badge>;
            case 'completed':
                return <Badge variant="default" className="gap-1 bg-green-600"><CheckCircle2 className="h-3 w-3" /> {t('dashboard.invites.statuses.completed')}</Badge>;
            case 'expired':
                return <Badge variant="destructive" className="gap-1"><XCircle className="h-3 w-3" /> {t('dashboard.invites.statuses.expired')}</Badge>;
            case 'timeout':
                return <Badge variant="outline" className="gap-1 border-amber-500 text-amber-600 dark:text-amber-400"><Clock className="h-3 w-3" /> {t('dashboard.invites.statuses.timeout')}</Badge>;
            case 'escaped':
                return <Badge variant="outline" className="gap-1 border-red-500 text-red-600 dark:text-red-400"><XCircle className="h-3 w-3" /> {t('dashboard.invites.statuses.escaped')}</Badge>;
            case 'needs_review':
                return <Badge variant="outline" className="gap-1 border-purple-500 text-purple-600 dark:text-purple-400"><AlertCircle className="h-3 w-3" /> {t('dashboard.invites.statuses.needs_review')}</Badge>;
            default:
                return <Badge variant="outline">{status}</Badge>;
        }
    };

    const getTestTitle = (testId: string) => {
        return allTests.find((t: Test) => t.id === testId)?.title || t('common.unknown');
    };

    // Stats
    const stats = useMemo(() => {
        if (!invites?.items) return { total: 0, pending: 0, completed: 0 };
        return {
            total: invites.items.length,
            pending: invites.items.filter(i => i.status === 'pending').length,
            completed: invites.items.filter(i => i.status === 'completed').length,
        };
    }, [invites]);

    const selectedTestObj = useMemo(() => {
        return allTests.find(t => t.id === selectedTest);
    }, [allTests, selectedTest]);

    return (
        <div className="space-y-6">
            {/* Header */}
            <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">{t('dashboard.invites.title')}</h1>
                    <p className="text-muted-foreground">
                        {t('dashboard.invites.subtitle')}
                    </p>
                </div>
            </div>

            {/* Stats Cards */}
            <div className="grid gap-4 md:grid-cols-3">
                <Card className="premium-hover">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">{t('dashboard.invites.stats.total')}</CardTitle>
                        <Send className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{stats.total}</div>
                    </CardContent>
                </Card>
                <Card className="premium-hover">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">{t('dashboard.invites.stats.pending')}</CardTitle>
                        <Clock className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{stats.pending}</div>
                    </CardContent>
                </Card>
                <Card className="premium-hover">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">{t('dashboard.invites.stats.completed')}</CardTitle>
                        <CheckCircle2 className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{stats.completed}</div>
                    </CardContent>
                </Card>
            </div>

            <div className="grid gap-6 lg:grid-cols-2">
                {/* Create Invite Form */}
                <Card className="premium-hover">
                    <CardHeader>
                        <CardTitle className="flex items-center gap-2">
                            <Send className="h-5 w-5" />
                            {t('dashboard.invites.create.title')}
                        </CardTitle>
                        <CardDescription>
                            {t('dashboard.invites.create.desc')}
                        </CardDescription>
                    </CardHeader>
                    <CardContent>
                        <form onSubmit={handleInvite} className="space-y-4">
                            <div className="space-y-2">
                                <Label>{t('dashboard.invites.create.select_test')}</Label>
                                {selectedTestObj ? (
                                    <div className="flex items-center gap-3 p-3 border rounded-lg bg-muted/50">
                                        <div className="h-10 w-10 rounded-full bg-primary/10 flex items-center justify-center">
                                            <FileText className="h-5 w-5 text-primary" />
                                        </div>
                                        <div className="flex-1 min-w-0">
                                            <p className="font-medium truncate">{selectedTestObj.title}</p>
                                            <p className="text-sm text-muted-foreground truncate">
                                                {selectedTestObj.duration_minutes} {t('dashboard.tests.duration')} • {Array.isArray(selectedTestObj.questions) ? selectedTestObj.questions.length : 0} {t('dashboard.tests.questions')}
                                            </p>
                                        </div>
                                        <Button
                                            type="button"
                                            variant="ghost"
                                            size="icon"
                                            onClick={() => setSelectedTest('')}
                                        >
                                            <X className="h-4 w-4" />
                                        </Button>
                                    </div>
                                ) : (
                                    <Select
                                        value={selectedTest}
                                        onValueChange={setSelectedTest}
                                        key={allTests.length || 'loading'}
                                    >
                                        <SelectTrigger>
                                            <SelectValue placeholder={t('dashboard.invites.create.select_test_placeholder')} />
                                        </SelectTrigger>
                                        <SelectContent>
                                            {/* Standard Tests Group */}
                                            {allTests.filter(t => !t.test_type || t.test_type === 'question_based').length > 0 && (
                                                <SelectGroup>
                                                    <SelectLabel className="flex items-center gap-2 text-[10px] uppercase tracking-wider text-muted-foreground/80 py-2 pin-0">
                                                        <ClipboardCheck className="h-3 w-3 text-emerald-500/70" />
                                                        {t('dashboard.tests.types.question_based')}
                                                    </SelectLabel>
                                                    {allTests
                                                        .filter(t => !t.test_type || t.test_type === 'question_based')
                                                        .sort((a, b) => a.title.localeCompare(b.title))
                                                        .map((test: Test) => (
                                                            <SelectItem key={test.id} value={test.id}>
                                                                {test.title}
                                                            </SelectItem>
                                                        ))}
                                                </SelectGroup>
                                            )}

                                            {allTests.filter(t => t.test_type === 'presentation').length > 0 && (
                                                <SelectGroup className="mt-1">
                                                    <SelectLabel className="flex items-center gap-2 text-[10px] uppercase tracking-wider text-muted-foreground/80 py-2 border-t pt-3">
                                                        <MonitorPlay className="h-3 w-3 text-blue-500/70" />
                                                        {t('dashboard.tests.types.presentation')}
                                                    </SelectLabel>
                                                    {allTests
                                                        .filter(t => t.test_type === 'presentation')
                                                        .sort((a, b) => a.title.localeCompare(b.title))
                                                        .map((test: Test) => (
                                                            <SelectItem key={test.id} value={test.id}>
                                                                {test.title}
                                                            </SelectItem>
                                                        ))}
                                                </SelectGroup>
                                            )}
                                        </SelectContent>
                                    </Select>
                                )}
                            </div>

                            {/* Candidate Search/Selection */}
                            <div className="space-y-2">
                                <Label>{t('dashboard.invites.create.candidate')}</Label>
                                {selectedCandidate ? (
                                    <div className="flex items-center gap-3 p-3 border rounded-lg bg-muted/50">
                                        <div className="h-10 w-10 rounded-full bg-primary/10 flex items-center justify-center">
                                            <User className="h-5 w-5 text-primary" />
                                        </div>
                                        <div className="flex-1 min-w-0">
                                            <p className="font-medium truncate">{selectedCandidate.name}</p>
                                            <p className="text-sm text-muted-foreground truncate">{selectedCandidate.email}</p>
                                        </div>
                                        <Button
                                            type="button"
                                            variant="ghost"
                                            size="icon"
                                            onClick={clearSelectedCandidate}
                                        >
                                            <X className="h-4 w-4" />
                                        </Button>
                                    </div>
                                ) : (
                                    <div className="relative" ref={dropdownRef}>
                                        <Input
                                            value={searchQuery}
                                            onChange={(e) => {
                                                setSearchQuery(e.target.value);
                                                setShowDropdown(true);
                                            }}
                                            onFocus={() => setShowDropdown(true)}
                                            placeholder={t('dashboard.invites.create.search_placeholder')}
                                        />
                                        {showDropdown && filteredCandidates.length > 0 && (
                                            <div className="absolute z-50 w-full mt-1 bg-popover border rounded-lg shadow-lg max-h-64 overflow-auto">
                                                {filteredCandidates.map((candidate: Candidate) => (
                                                    <button
                                                        key={candidate.id}
                                                        type="button"
                                                        className="w-full flex items-center gap-3 p-3 hover:bg-muted/50 text-left transition-colors"
                                                        onClick={() => selectCandidate(candidate)}
                                                    >
                                                        <div className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center flex-shrink-0">
                                                            <User className="h-4 w-4 text-primary" />
                                                        </div>
                                                        <div className="flex-1 min-w-0">
                                                            <p className="font-medium text-sm truncate">{candidate.name}</p>
                                                            <div className="flex items-center gap-3 text-xs text-muted-foreground">
                                                                <span className="flex items-center gap-1 truncate">
                                                                    <Mail className="h-3 w-3" />
                                                                    {candidate.email}
                                                                </span>
                                                                {candidate.telegram_id && (
                                                                    <span className="flex items-center gap-1">
                                                                        <AtSign className="h-3 w-3" />
                                                                        {candidate.telegram_id}
                                                                    </span>
                                                                )}
                                                            </div>
                                                        </div>
                                                    </button>
                                                ))}
                                            </div>
                                        )}
                                        {showDropdown && searchQuery && filteredCandidates.length === 0 && (
                                            <div className="absolute z-50 w-full mt-1 bg-popover border rounded-lg shadow-lg p-4 text-center text-sm text-muted-foreground">
                                                {t('dashboard.invites.create.not_found')}
                                            </div>
                                        )}
                                    </div>
                                )}
                            </div>

                            <Button
                                type="submit"
                                className="w-full"
                                disabled={inviteMutation.isPending || !selectedCandidate}
                            >
                                {inviteMutation.isPending ? t('dashboard.invites.create.sending') : t('dashboard.invites.create.submit_btn')}
                            </Button>
                        </form>
                    </CardContent>
                </Card>

                {/* Recent Invites */}
                <Card className="premium-hover">
                    <CardHeader>
                        <CardTitle className="flex items-center gap-2">
                            <FileText className="h-5 w-5" />
                            {t('dashboard.invites.recent.title')}
                        </CardTitle>
                        <CardDescription>
                            {t('dashboard.invites.recent.desc')}
                        </CardDescription>
                    </CardHeader>
                    <CardContent>
                        {invitesLoading ? (
                            <div className="text-center py-8 text-muted-foreground">
                                {t('dashboard.invites.recent.loading')}
                            </div>
                        ) : invites?.items && invites.items.length > 0 ? (
                            <div className="space-y-3">
                                {invites.items.slice(0, 5).map((invite) => (
                                    <div
                                        key={invite.id}
                                        className="flex items-center justify-between p-3 rounded-lg border bg-card hover:bg-muted/50 transition-colors"
                                    >
                                        <div className="flex items-center gap-3 min-w-0">
                                            <div className="h-9 w-9 rounded-full bg-primary/10 flex items-center justify-center flex-shrink-0">
                                                <User className="h-4 w-4 text-primary" />
                                            </div>
                                            <div className="min-w-0">
                                                <p className="font-medium text-sm truncate">{invite.candidate_name}</p>
                                                <p className="text-xs text-muted-foreground truncate">
                                                    {getTestTitle(invite.test_id)}
                                                </p>
                                            </div>
                                        </div>
                                        <div className="flex items-center gap-2 flex-shrink-0">
                                            {getStatusBadge(invite.status)}
                                            {invite.status === 'pending' && (
                                                <AlertDialog>
                                                    <AlertDialogTrigger asChild>
                                                        <Button
                                                            variant="ghost"
                                                            size="icon"
                                                            className="h-8 w-8 text-destructive hover:text-destructive hover:bg-destructive/10"
                                                            disabled={deleteMutation.isPending}
                                                        >
                                                            {deleteMutation.isPending && deleteMutation.variables === invite.id ? (
                                                                <Loader2 className="h-3 w-3 animate-spin" />
                                                            ) : (
                                                                <Trash2 className="h-3 w-3" />
                                                            )}
                                                        </Button>
                                                    </AlertDialogTrigger>
                                                    <AlertDialogContent>
                                                        <AlertDialogHeader>
                                                            <AlertDialogTitle>{t('dashboard.attempts.delete_confirm')}</AlertDialogTitle>
                                                            <AlertDialogDescription>
                                                                {t('dashboard.tests.delete_desc')}
                                                            </AlertDialogDescription>
                                                        </AlertDialogHeader>
                                                        <AlertDialogFooter>
                                                            <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
                                                            <AlertDialogAction
                                                                onClick={() => handleDelete(invite.id)}
                                                                className="bg-destructive hover:bg-destructive/90"
                                                            >
                                                                {t('common.delete')}
                                                            </AlertDialogAction>
                                                        </AlertDialogFooter>
                                                    </AlertDialogContent>
                                                </AlertDialog>
                                            )}
                                        </div>
                                    </div>
                                ))}
                            </div>
                        ) : (
                            <div className="text-center py-8 border-2 border-dashed rounded-lg">
                                <Users className="h-8 w-8 mx-auto text-muted-foreground mb-2" />
                                <p className="text-sm text-muted-foreground">
                                    {t('dashboard.invites.recent.empty')}
                                </p>
                                <p className="text-xs text-muted-foreground mt-1">
                                    {t('dashboard.invites.recent.empty_desc')}
                                </p>
                            </div>
                        )}
                    </CardContent>
                </Card>
            </div>
        </div>
    );
}

export default function InvitesPage() {
    return (
        <Suspense fallback={<div className="p-6">Loading...</div>}>
            <InvitesPageContent />
        </Suspense>
    );
}
