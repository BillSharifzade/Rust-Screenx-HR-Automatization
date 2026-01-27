'use client';

import { useState, useEffect, useCallback } from 'react';
import { useParams, useRouter, useSearchParams } from 'next/navigation';
import { useQuery, useMutation } from '@tanstack/react-query';
import { apiFetch } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { toast } from 'sonner';
import { Clock, AlertCircle, CheckCircle2, PlayCircle, Loader2, Upload, FileCheck, Trash2, Link2, ListChecks, Presentation } from 'lucide-react';
import { cn, FormattedText } from '@/lib/utils';
import { useTranslation } from '@/lib/i18n-context';
import { LanguageToggle } from '@/components/language-toggle';
import { ModeToggle } from '@/components/mode-toggle';

// Types (should actully be in generic types file)
interface Question {
    id: number;
    type: 'multiple_choice' | 'short_answer' | 'code';
    question: string;
    options?: string[];
    min_words?: number;
}

interface TestData {
    test: {
        title: string;
        description: string;
        instructions: string;
        duration_minutes: number;
        total_questions: number;
        test_type?: string;
        presentation_themes?: string[];
        presentation_extra_info?: string;
    };
    attempt: {
        id: string;
        status: string;
        expires_at: string;
        candidate_name: string;
        candidate_external_id?: string;
    };
}

interface StartResponse {
    attempt_id: string;
    status: string;
    started_at: string;
    expires_at: string;
    questions: Question[]; // Snapshot
}

// Imports from original file plus Input, File
import { Input } from '@/components/ui/input';

export default function TestPage() {
    const { t } = useTranslation();
    const params = useParams();
    const router = useRouter();
    const searchParams = useSearchParams();
    const token = params.token as string;
    const autostart = searchParams.get('autostart') === 'true';

    // State
    const [started, setStarted] = useState(false);
    const [questions, setQuestions] = useState<Question[]>([]);
    const [currentQuestionIndex, setCurrentQuestionIndex] = useState(0);
    const [answers, setAnswers] = useState<Record<number, any>>({});
    const [timeLeftSeconds, setTimeLeftSeconds] = useState<number | null>(null);
    const [isSubmitting, setIsSubmitting] = useState(false);

    // Presentation State
    const [presentationLink, setPresentationLink] = useState('');
    const [presentationFile, setPresentationFile] = useState<File | null>(null);

    // Initial Data Fetch
    const { data: testData, isLoading: isTestLoading, error: testError } = useQuery({
        queryKey: ['test', token],
        queryFn: () => apiFetch<TestData>(`/api/public/tests/${token}`),
        retry: false,
    });

    // Start Test Mutation
    const startMutation = useMutation({
        mutationFn: () => apiFetch<StartResponse>(`/api/public/tests/${token}/start`, { method: 'POST' }),
        onSuccess: (data) => {
            setStarted(true);
            setQuestions(data.questions || []);
            // Initialize timer
            const now = new Date();
            const expires = new Date(data.expires_at);
            const secondsRemaining = Math.max(0, Math.floor((expires.getTime() - now.getTime()) / 1000));
            setTimeLeftSeconds(secondsRemaining);
        },
        onError: (err: any) => {
            const msg = err.message || t('test.error_load');
            toast.error(msg);
        }
    });

    // Handle session end/re-entry
    useEffect(() => {
        if (!testData) return;
        const { attempt } = testData;

        if ((attempt.status === 'timeout' || attempt.status === 'escaped') && !isSubmitting) {
            const candidateId = attempt.candidate_external_id;
            if (candidateId) {
                const msg = attempt.status === 'timeout'
                    ? t('test.timeout_msg')
                    : t('test.escaped_msg');
                toast.info(msg);
                router.push(`/candidate/${candidateId}`);
            } else {
                router.push(`/test/${token}/result`);
            }
        }
    }, [testData, isSubmitting]);

    // Answer Save Mutation (Question-based only)
    const answerMutation = useMutation({
        mutationFn: (payload: { question_id: number; answer: any }) => {
            let finalAnswer = payload.answer;
            const q = questions.find(q => q.id === payload.question_id);
            if (q?.type === 'multiple_choice') {
                finalAnswer = parseInt(payload.answer as string);
            }

            return apiFetch(`/api/public/tests/${token}/answer`, {
                method: 'PATCH',
                body: JSON.stringify({
                    question_id: payload.question_id,
                    answer: finalAnswer,
                    time_spent_seconds: 0,
                }),
            });
        }
    });

    // Submit Mutation (Question-based)
    const submitMutation = useMutation({
        mutationFn: (overrideStatus?: string) => {
            // Validate minimum answer length for short_answer questions (20 chars)
            if (!overrideStatus) {
                for (const q of questions) {
                    const ans = answers[q.id];
                    if (ans === undefined || ans === null || (typeof ans === 'string' && ans.trim() === '')) {
                        throw new Error(t('test.answer_required'));
                    }
                    if (q.type === 'short_answer') {
                        const answer = answers[q.id] || '';
                        if (answer.trim().length < 20) {
                            throw new Error(t('test.answer_too_short'));
                        }
                    }
                }
            }

            const payload = Object.entries(answers).map(([qidStr, val]) => {
                const qid = parseInt(qidStr);
                const q = questions.find(q => q.id === qid);
                let finalAnswer = val;
                if (q?.type === 'multiple_choice') {
                    finalAnswer = parseInt(val as string);
                }
                return {
                    question_id: qid,
                    answer: finalAnswer,
                    time_spent_seconds: 0
                };
            });

            return apiFetch(`/api/public/tests/${token}/submit`, {
                method: 'POST',
                body: JSON.stringify({
                    answers: payload,
                    status: overrideStatus
                })
            });
        },
        onSuccess: () => {
            setIsSubmitting(true);
            router.push(`/test/${token}/result`);
        },
        onError: (err: any) => {
            const msg = err.message || t('test.submit_error');
            toast.error(msg);
            setIsSubmitting(false);
        }
    });

    // Submit Presentation Mutation
    const submitPresentationMutation = useMutation({
        mutationFn: () => {
            const allowedFileExtensions = ['.pdf', '.pptx', '.ppt', '.key'];

            if (presentationLink) {
                try {
                    const url = new URL(presentationLink);
                    if (url.protocol !== 'http:' && url.protocol !== 'https:') {
                        throw new Error(t('dashboard.attempts.presentation.invalid_url_scheme'));
                    }
                } catch {
                    throw new Error(t('dashboard.attempts.presentation.invalid_url'));
                }
            }

            if (presentationFile) {
                const ext = presentationFile.name.toLowerCase().substring(presentationFile.name.lastIndexOf('.'));
                if (!allowedFileExtensions.includes(ext)) {
                    throw new Error(
                        t('dashboard.attempts.presentation.invalid_file_type')
                            .replace('{types}', allowedFileExtensions.join(", "))
                    );
                }
            }

            const formData = new FormData();
            if (presentationLink) formData.append('presentation_link', presentationLink);
            if (presentationFile) formData.append('file', presentationFile);

            if (!presentationLink && !presentationFile) {
                throw new Error(t('dashboard.attempts.presentation.missing_submission'));
            }

            return apiFetch(`/api/public/tests/${token}/submit-presentation`, {
                method: 'POST',
                body: formData,
                headers: {}, // Let browser set Content-Type for FormData
            });
        },
        onSuccess: () => {
            setIsSubmitting(true);
            toast.success(t('dashboard.attempts.presentation.submission_success'));
            router.push(`/test/${token}/result`);
        },
        onError: (err: any) => {
            toast.error(err.message || t('test.submit_error'));
        }
    });

    // Abandon Logic
    const escapeTest = useCallback(() => {
        if (isSubmitting || !started) return;
        setIsSubmitting(true);

        try {
            const payload = Object.entries(answers).map(([qidStr, val]) => {
                const qid = parseInt(qidStr);
                const q = questions.find(q => q.id === qid);
                let finalAnswer = val;
                if (q?.type === 'multiple_choice') {
                    finalAnswer = parseInt(val as string);
                }
                return {
                    question_id: qid,
                    answer: finalAnswer,
                    time_spent_seconds: 0
                };
            });

            const url = `${process.env.NEXT_PUBLIC_API_URL || ''}/api/public/tests/${token}/submit`;
            const data = {
                answers: payload,
                status: "escaped"
            };

            // Try sendBeacon first (most reliable for page close)
            const blob = new Blob([JSON.stringify(data)], { type: 'application/json' });
            if (!navigator.sendBeacon(url, blob)) {
                // Fallback to fetch if beacon fails (e.g. data too large)
                fetch(url, {
                    method: 'POST',
                    keepalive: true,
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(data)
                }).catch(e => console.error("Escape fetch failed", e));
            }
        } catch (e) {
            console.error("Escape error", e);
        }
    }, [isSubmitting, started, answers, questions, token]);

    // Auto-start if already active OR if autostart query param is set (Task 15: skip intro when coming from modal)
    useEffect(() => {
        if (!testData || started || startMutation.isPending) return;

        const shouldAutostart =
            testData.attempt.status === 'in_progress' || // Resume in-progress tests
            (autostart && testData.attempt.status === 'pending'); // Auto-start from modal

        if (shouldAutostart) {
            startMutation.mutate();
        }
    }, [testData?.attempt.status, autostart]);

    // Heartbeat Logic (for robust abandon detection)
    useEffect(() => {
        if (!started || isSubmitting || !testData || testData.test.test_type === 'presentation') return;

        const sendHeartbeat = () => {
            const url = `${process.env.NEXT_PUBLIC_API_URL || ''}/api/public/tests/${token}/heartbeat`;
            // Standard fetch is fine, we just need to touch the server
            fetch(url, { method: 'POST', keepalive: false })
                .catch(e => console.error("Heartbeat failed", e));
        };

        // Initial heartbeat
        sendHeartbeat();

        // Send every 30 seconds
        const interval = setInterval(sendHeartbeat, 30000);
        return () => clearInterval(interval);
    }, [started, isSubmitting, testData, token]);

    // Timer Logic
    useEffect(() => {
        if (!started || timeLeftSeconds === null) return;

        if (timeLeftSeconds <= 0) {
            // Time is up!
            toast.warning(t('test.times_up'));
            // For presentation, we might not auto-submit? Or we should.
            // Let's assume strict deadline for presentation too.
            if (testData?.test.test_type === 'presentation') {
                // For presentation, just warn or mark as timeout if necessary.
                // The requirement says "if hes late, give it status time out".
                // But presentation submission is typically manual.
            } else {
                submitMutation.mutate("timeout");
            }
            return;
        }

        const interval = setInterval(() => {
            setTimeLeftSeconds((prev) => Math.max(0, (prev || 0) - 1));
        }, 1000);

        return () => clearInterval(interval);
    }, [started, timeLeftSeconds]);

    // Exit Detection
    useEffect(() => {
        const handleFlush = () => {
            if (isSubmitting || !started || testData?.test.test_type === 'presentation') return;
            escapeTest();
        };

        const handleBeforeUnload = (e: BeforeUnloadEvent) => {
            if (isSubmitting || !started || testData?.test.test_type === 'presentation') return;
            handleFlush();
            e.preventDefault();
            e.returnValue = '';
        };

        window.addEventListener('beforeunload', handleBeforeUnload);
        window.addEventListener('pagehide', handleFlush);

        return () => {
            window.removeEventListener('beforeunload', handleBeforeUnload);
            window.removeEventListener('pagehide', handleFlush);
        };
    }, [isSubmitting, started, testData, escapeTest]);

    const formatTime = (seconds: number) => {
        const mins = Math.floor(seconds / 60);
        const secs = seconds % 60;
        if (mins > 60) {
            const hours = Math.floor(mins / 60);
            return `${hours}h ${mins % 60}m`;
        }
        return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
    };

    if (isTestLoading || (started && questions.length === 0 && testData?.test.test_type !== 'presentation') || (autostart && !started && !testError)) {
        return (
            <div className="flex h-screen w-full items-center justify-center bg-muted/20">
                <div className="text-center space-y-4">
                    <Loader2 className="h-10 w-10 animate-spin text-primary mx-auto" />
                    <p className="text-muted-foreground">{t('test.loading')}</p>
                </div>
            </div>
        );
    }

    if (testError || !testData) {
        return (
            <div className="flex h-screen items-center justify-center bg-muted/20 p-4">
                {/* Error Card */}
                <Card className="max-w-md w-full border-destructive/50">
                    <CardHeader>
                        <CardTitle className="flex items-center gap-2 text-destructive">
                            <AlertCircle className="h-5 w-5" />
                            {t('test.error_load')}
                        </CardTitle>
                    </CardHeader>
                    <CardContent>
                        <p>{t('test.error_load')}</p>
                        <Button className="mt-4 w-full" variant="outline" onClick={() => router.push('/')}>
                            {t('test.return_home')}
                        </Button>
                    </CardContent>
                </Card>
            </div>
        );
    }

    const { test, attempt } = testData;
    const isPresentation = test.test_type === 'presentation';

    // Intro Screen
    if (!started) {
        return (
            <div className="min-h-screen bg-muted/20 flex items-center justify-center p-4 relative">
                <div className="absolute top-4 right-4 flex items-center gap-2">
                    <LanguageToggle variant="inline" />
                    <ModeToggle />
                </div>
                <Card className="max-w-2xl w-full shadow-lg">
                    <CardHeader className="text-center py-8 bg-primary/5">
                        <CardTitle className="text-3xl font-bold text-primary">{test.title}</CardTitle>
                        <FormattedText text={test.description} className="text-muted-foreground mt-2 text-lg" />
                        {isPresentation && <p className='text-sm text-primary font-medium mt-1 uppercase tracking-widest'>{t('dashboard.attempts.presentation.title')}</p>}
                        {attempt.status === 'in_progress' && (
                            <div className="mt-4 p-2 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg text-blue-700 dark:text-blue-300 text-sm font-medium">
                                {t('test.in_progress_note') || "You have an ongoing attempt."}
                            </div>
                        )}
                    </CardHeader>
                    <CardContent className="space-y-8 pt-8">
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                            <div className="flex items-center gap-4 p-4 rounded-xl bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-300">
                                <Clock className="h-8 w-8" />
                                <div>
                                    <p className="text-xs opacity-80 font-medium uppercase tracking-wider">{t('test.duration')}</p>
                                    <p className="text-2xl font-bold">
                                        {isPresentation
                                            ? `${(test.duration_minutes / 60).toFixed(0)} ${t('dashboard.attempts.presentation.hours')}`
                                            : `${test.duration_minutes} ${t('test.duration_unit')}`
                                        }
                                    </p>
                                </div>
                            </div>
                            <div className="flex items-center gap-4 p-4 rounded-xl bg-amber-50 dark:bg-amber-900/20 text-amber-700 dark:text-amber-300">
                                <ListChecks className="h-8 w-8" />
                                <div>
                                    <p className="text-xs opacity-80 font-medium uppercase tracking-wider">
                                        {isPresentation ? t('dashboard.attempts.presentation.themes') : t('test.questions')}
                                    </p>
                                    <p className="text-2xl font-bold">
                                        {isPresentation
                                            ? test.presentation_themes?.length || 0
                                            : test.total_questions || 0
                                        }
                                    </p>
                                </div>
                            </div>
                        </div>

                        <div className="space-y-4 rounded-lg border p-6 bg-card">
                            <h3 className="font-semibold flex items-center gap-2">
                                <CheckCircle2 className="h-5 w-5 text-green-600" />
                                {t('test.instructions')}
                            </h3>
                            <div className="prose prose-sm dark:prose-invert max-w-none text-muted-foreground">
                                <FormattedText text={test.instructions || t('test.default_instructions')} />
                            </div>
                        </div>

                        <div className="pt-4">
                            <Button
                                size="lg"
                                className="w-full text-lg h-14"
                                onClick={() => startMutation.mutate()}
                                disabled={startMutation.isPending}
                            >
                                {startMutation.isPending ? (
                                    <>
                                        <Loader2 className="mr-2 h-5 w-5 animate-spin" />
                                        {t('common.loading')}
                                    </>
                                ) : (
                                    <>
                                        {attempt.status === 'in_progress'
                                            ? (isPresentation ? t('test.continue_presentation') : t('test.continue_test'))
                                            : (isPresentation ? t('dashboard.attempts.presentation.start_button') : t('test.start_btn'))
                                        }
                                        <PlayCircle className="ml-2 h-5 w-5" />
                                    </>
                                )}
                            </Button>
                            {!isPresentation && (
                                <p className="text-xs text-center text-muted-foreground mt-4">
                                    {t('test.exit_note')}
                                </p>
                            )}
                        </div>
                    </CardContent>
                </Card>
            </div>
        );
    }

    if (isPresentation) {
        return (
            <div className="min-h-screen bg-gradient-to-b from-muted/30 to-background flex flex-col">
                <header className="sticky top-0 z-10 bg-background/95 backdrop-blur border-b shadow-sm">
                    <div className="container mx-auto px-4 h-16 flex items-center justify-between gap-4">
                        <div className="flex items-center gap-3 overflow-hidden min-w-0 flex-1">
                            <div className="h-8 w-8 rounded-full bg-purple-600 flex items-center justify-center shrink-0">
                                <Presentation className="h-4 w-4 text-white" />
                            </div>
                            <h1 className="font-semibold text-base sm:text-lg truncate">{test.title}</h1>
                        </div>
                        <div className="flex items-center gap-3 shrink-0">
                            <div className="flex items-center gap-1 sm:gap-2">
                                <LanguageToggle variant="inline" />
                                <ModeToggle />
                            </div>
                            <div className={cn(
                                "font-mono text-sm sm:text-base font-bold flex items-center gap-2 px-3 py-2 rounded-lg border shadow-sm whitespace-nowrap",
                                (timeLeftSeconds || 0) < 3600
                                    ? "bg-red-50 dark:bg-red-900/30 text-red-600 dark:text-red-400 border-red-200 animate-pulse"
                                    : "bg-muted/50 border-input text-foreground"
                            )}>
                                <Clock className="h-4 w-4 shrink-0" />
                                <span>{timeLeftSeconds !== null ? formatTime(timeLeftSeconds) : '--:--'}</span>
                            </div>
                        </div>
                    </div>
                </header>

                <main className="container mx-auto max-w-3xl py-8 px-4 space-y-6">
                    {/* Themes Card */}
                    <Card className="border-purple-200 dark:border-purple-800/50 shadow-lg">
                        <CardHeader className="bg-purple-50 dark:bg-purple-900/20">
                            <CardTitle className="flex items-center gap-2 text-purple-700 dark:text-purple-300">
                                <ListChecks className="h-5 w-5" />
                                {t('dashboard.attempts.presentation.themes')}
                            </CardTitle>
                        </CardHeader>
                        <CardContent className="pt-6">
                            <div className="space-y-3">
                                {test.presentation_themes?.map((theme: string, i: number) => (
                                    <div key={i} className="flex items-start gap-3 p-4 rounded-lg bg-muted/50 hover:bg-muted transition-colors">
                                        <div className="h-6 w-6 rounded-full bg-purple-600 text-white flex items-center justify-center text-sm font-semibold shrink-0">
                                            {i + 1}
                                        </div>
                                        <span className="text-base">{theme}</span>
                                    </div>
                                ))}
                            </div>
                        </CardContent>
                    </Card>

                    {/* Extra Info Card */}
                    {test.presentation_extra_info && (
                        <Card>
                            <CardHeader>
                                <CardTitle className="flex items-center gap-2">
                                    <AlertCircle className="h-5 w-5 text-blue-600" />
                                    {t('dashboard.attempts.presentation.extra_info')}
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <div className="prose prose-sm dark:prose-invert max-w-none">
                                    <FormattedText text={test.presentation_extra_info} className="text-muted-foreground" />
                                </div>
                            </CardContent>
                        </Card>
                    )}

                    {/* Submission Card */}
                    <Card className="border-green-200 dark:border-green-800/50 shadow-lg">
                        <CardHeader className="bg-green-50 dark:bg-green-900/20">
                            <CardTitle className="flex items-center gap-2 text-green-700 dark:text-green-300">
                                <CheckCircle2 className="h-5 w-5" />
                                {t('dashboard.attempts.presentation.submit_title')}
                            </CardTitle>
                            <p className="text-sm text-muted-foreground mt-1">
                                {t('dashboard.attempts.presentation.submit_desc')}
                            </p>
                        </CardHeader>
                        <CardContent className="pt-6 space-y-6">
                            {/* Link Input */}
                            <div className="space-y-3">
                                <Label className="text-base font-medium flex items-center gap-2">
                                    <Link2 className="h-4 w-4" />
                                    {t('dashboard.attempts.presentation.link_label')}
                                </Label>
                                <Input
                                    placeholder={t('dashboard.attempts.presentation.link_placeholder')}
                                    value={presentationLink}
                                    onChange={(e) => setPresentationLink(e.target.value)}
                                    className="h-12 text-base"
                                />
                                {(() => {
                                    if (!presentationLink) return null;
                                    try {
                                        const url = new URL(presentationLink);
                                        if (url.protocol === 'http:' || url.protocol === 'https:') {
                                            return (
                                                <div className="flex items-center gap-2 text-sm text-green-600 dark:text-green-400 animate-in fade-in slide-in-from-left-2">
                                                    <CheckCircle2 className="h-4 w-4" />
                                                    <span>{t('dashboard.attempts.presentation.link_added')}</span>
                                                </div>
                                            );
                                        }
                                    } catch {
                                        return (
                                            <div className="flex items-center gap-2 text-sm text-destructive animate-in fade-in slide-in-from-left-2">
                                                <AlertCircle className="h-4 w-4" />
                                                <span>{t('dashboard.attempts.presentation.invalid_url')}</span>
                                            </div>
                                        );
                                    }
                                    return null;
                                })()}
                                {/* Public Link Warning */}
                                <div className="flex items-start gap-3 p-4 rounded-lg bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800">
                                    <AlertCircle className="h-5 w-5 text-amber-600 shrink-0 mt-0.5" />
                                    <p className="text-sm text-amber-800 dark:text-amber-200">
                                        {t('dashboard.attempts.presentation.public_link_warning')}
                                    </p>
                                </div>
                            </div>

                            {/* Divider */}
                            <div className="relative py-2">
                                <div className="absolute inset-0 flex items-center">
                                    <span className="w-full border-t border-dashed" />
                                </div>
                                <div className="relative flex justify-center">
                                    <span className="bg-background px-4 text-sm font-medium text-muted-foreground uppercase tracking-wider">
                                        {t('dashboard.attempts.presentation.or_divider')}
                                    </span>
                                </div>
                            </div>

                            {/* File Upload */}
                            <div className="space-y-3">
                                <Label className="text-base font-medium flex items-center gap-2">
                                    <Upload className="h-4 w-4" />
                                    {t('dashboard.attempts.presentation.file_label')}
                                </Label>
                                <div className={cn(
                                    "border-2 border-dashed rounded-xl p-6 transition-all cursor-pointer hover:border-primary/50 hover:bg-muted/50",
                                    presentationFile ? "border-green-500 bg-green-50 dark:bg-green-900/20" : "border-muted-foreground/30"
                                )}>
                                    <input
                                        type="file"
                                        accept=".pdf,.pptx,.ppt,.key"
                                        onChange={(e) => setPresentationFile(e.target.files?.[0] || null)}
                                        className="hidden"
                                        id="presentation-file"
                                    />
                                    <label htmlFor="presentation-file" className="cursor-pointer flex flex-col items-center gap-3">
                                        {presentationFile ? (
                                            <>
                                                <div className="h-12 w-12 rounded-full bg-green-100 dark:bg-green-900/50 flex items-center justify-center">
                                                    <FileCheck className="h-6 w-6 text-green-600 dark:text-green-400" />
                                                </div>
                                                <div className="text-center">
                                                    <p className="font-medium text-green-700 dark:text-green-300">{presentationFile.name}</p>
                                                    <p className="text-sm text-muted-foreground">
                                                        {(presentationFile.size / 1024 / 1024).toFixed(2)} MB
                                                    </p>
                                                </div>
                                                <Button variant="ghost" size="sm" onClick={(e) => { e.preventDefault(); setPresentationFile(null); }}>
                                                    <Trash2 className="h-4 w-4 mr-2" />
                                                    {t('dashboard.attempts.presentation.remove_file')}
                                                </Button>
                                            </>
                                        ) : (
                                            <>
                                                <div className="h-12 w-12 rounded-full bg-muted flex items-center justify-center">
                                                    <Upload className="h-6 w-6 text-muted-foreground" />
                                                </div>
                                                <div className="text-center">
                                                    <p className="font-medium">{t('dashboard.attempts.presentation.click_to_upload')}</p>
                                                    <p className="text-sm text-muted-foreground">
                                                        {t('dashboard.attempts.presentation.file_placeholder')}
                                                    </p>
                                                </div>
                                            </>
                                        )}
                                    </label>
                                </div>
                            </div>

                            {/* Submit Button */}
                            <Button
                                size="lg"
                                className="w-full h-14 text-lg bg-green-600 hover:bg-green-700"
                                onClick={() => submitPresentationMutation.mutate()}
                                disabled={submitPresentationMutation.isPending || (!presentationLink && !presentationFile)}
                            >
                                {submitPresentationMutation.isPending ? (
                                    <>
                                        <Loader2 className="mr-2 h-5 w-5 animate-spin" />
                                        {t('dashboard.attempts.presentation.submitting')}
                                    </>
                                ) : (
                                    <>
                                        <CheckCircle2 className="mr-2 h-5 w-5" />
                                        {t('dashboard.attempts.presentation.finish_button')}
                                    </>
                                )}
                            </Button>
                        </CardContent>
                    </Card>
                </main>
            </div>
        );
    }

    // Active Test Screen (Question Based)
    const currentQ = questions[currentQuestionIndex];
    if (!currentQ) return null;

    const timerColor = (timeLeftSeconds || 0) < 60 ? 'text-red-600 animate-pulse' : 'text-primary';

    return (
        <div className="min-h-screen bg-muted/20 flex flex-col">
            {/* Header / TimerBar */}
            <header className="sticky top-0 z-10 bg-background/95 backdrop-blur border-b shadow-sm">
                <div className="container mx-auto px-4 h-16 flex items-center justify-between">
                    <h1 className="font-semibold truncate max-w-[200px] sm:max-w-md hidden sm:block">
                        {test.title}
                    </h1>
                    <div className="flex items-center gap-4 w-full sm:w-auto justify-between sm:justify-end">
                        <div className="flex items-center gap-2 mr-2">
                            <LanguageToggle variant="inline" />
                            <ModeToggle />
                        </div>
                        <div className="text-sm font-medium text-muted-foreground">
                            {t('test.question')} {currentQuestionIndex + 1} {t('test.of')} {questions.length}
                        </div>
                        <div className={cn("font-mono text-xl font-bold flex items-center justify-center gap-2 w-32 px-3 py-1 rounded-md bg-muted", timerColor)}>
                            <Clock className="h-5 w-5" />
                            {timeLeftSeconds !== null ? formatTime(timeLeftSeconds) : '--:--'}
                        </div>
                    </div>
                </div>
            </header>

            {/* Main Content */}
            <main className="flex-1 container mx-auto px-4 py-8 mb-24 max-w-4xl">
                <Card className="shadow-md border-primary/10">
                    <CardHeader className="bg-muted/30 pb-6">
                        <div className="flex justify-between items-start gap-4">
                            <div className="space-y-1">
                                <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                                    {t(`test.types.${currentQ.type}` as any) || currentQ.type.replace('_', ' ')}
                                </span>
                                <CardTitle className="text-xl leading-relaxed">
                                    <FormattedText text={currentQ.question} />
                                </CardTitle>
                            </div>
                        </div>
                    </CardHeader>
                    <CardContent className="pt-8 pb-8 space-y-8">
                        {currentQ.type === 'multiple_choice' && currentQ.options && (
                            <RadioGroup
                                key={currentQ.id}
                                value={answers[currentQ.id] ?? ""}
                                onValueChange={(val) => {
                                    setAnswers(prev => ({ ...prev, [currentQ.id]: val }));
                                    answerMutation.mutate({ question_id: currentQ.id, answer: val });
                                }}
                                className="space-y-4"
                            >
                                {currentQ.options.map((opt, idx) => (
                                    <div key={idx} className={cn(
                                        "flex items-center space-x-3 p-4 rounded-lg border transition-all hover:bg-muted/50 cursor-pointer",
                                        answers[currentQ.id] === (idx).toString() ? "border-primary bg-primary/5 ring-1 ring-primary" : "border-transparent bg-muted/20"
                                    )}>
                                        <RadioGroupItem value={idx.toString()} id={`opt-${idx}`} className="h-5 w-5 mt-0.5" />
                                        <Label htmlFor={`opt-${idx}`} className="flex-1 cursor-pointer text-base font-normal leading-relaxed">
                                            {opt}
                                        </Label>
                                    </div>
                                ))}
                            </RadioGroup>
                        )}

                        {currentQ.type === 'short_answer' && (
                            <div className="space-y-4" key={currentQ.id}>
                                <Textarea
                                    placeholder={t('test.placeholder_answer')}
                                    className="min-h-[200px] text-lg p-4 resize-y font-normal"
                                    value={answers[currentQ.id] ?? ""}
                                    onChange={(e) => {
                                        const val = e.target.value;
                                        setAnswers(prev => ({ ...prev, [currentQ.id]: val }));
                                    }}
                                    onBlur={(e) => {
                                        answerMutation.mutate({ question_id: currentQ.id, answer: e.target.value });
                                    }}
                                />
                                {currentQ.min_words && (
                                    <p className="text-xs text-muted-foreground text-right">
                                        {t('test.min_words').replace('{count}', currentQ.min_words.toString())}
                                    </p>
                                )}
                            </div>
                        )}
                    </CardContent>
                </Card>
            </main>

            {/* Navigation Footer */}
            <div className="fixed bottom-0 left-0 right-0 bg-background border-t p-4 z-20 shadow-[-10px_0_30px_-5px_hsl(var(--foreground)/0.1)]">
                <div className="container mx-auto max-w-4xl flex justify-between items-center">
                    <Button
                        variant="outline"
                        size="lg"
                        onClick={() => setCurrentQuestionIndex(Math.max(0, currentQuestionIndex - 1))}
                        disabled={currentQuestionIndex === 0}
                        className="w-32"
                    >
                        {t('test.previous')}
                    </Button>

                    <div className="hidden sm:flex gap-1 overflow-x-auto max-w-[200px] justify-center">
                        {questions.map((_, i) => (
                            <div
                                key={i}
                                className={cn(
                                    "h-2 w-2 rounded-full shrink-0",
                                    i === currentQuestionIndex ? "bg-primary" :
                                        answers[questions[i].id] ? "bg-primary/40" : "bg-muted-foreground/20"
                                )}
                            />
                        ))}
                    </div>

                    {currentQuestionIndex < questions.length - 1 ? (
                        <Button
                            size="lg"
                            className="w-32"
                            onClick={() => {
                                const q = questions[currentQuestionIndex];
                                const ans = answers[q.id];
                                if (ans === undefined || ans === null || (typeof ans === 'string' && ans.trim() === '')) {
                                    toast.error(t('test.answer_required'));
                                    return;
                                }
                                if (q.type === 'short_answer' && String(ans).trim().length < 20) {
                                    toast.error(t('test.answer_too_short'));
                                    return;
                                }
                                setCurrentQuestionIndex(currentQuestionIndex + 1);
                            }}
                        >
                            {t('test.next')}
                        </Button>
                    ) : (
                        <Button
                            size="lg"
                            className="w-32 bg-green-600 hover:bg-green-700 text-white"
                            onClick={() => {
                                const q = questions[currentQuestionIndex];
                                const ans = answers[q.id];
                                if (ans === undefined || ans === null || (typeof ans === 'string' && ans.trim() === '')) {
                                    toast.error(t('test.answer_required'));
                                    return;
                                }
                                if (q.type === 'short_answer' && String(ans).trim().length < 20) {
                                    toast.error(t('test.answer_too_short'));
                                    return;
                                }
                                submitMutation.mutate(undefined);
                            }}
                            disabled={submitMutation.isPending}
                        >
                            {submitMutation.isPending ? (
                                <Loader2 className="h-4 w-4 animate-spin" />
                            ) : t('test.submit')}
                        </Button>
                    )}
                </div>
            </div>
        </div >
    );
}
