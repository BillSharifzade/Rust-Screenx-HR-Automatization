"use client"

import { useState, useRef } from "react"
import { User, Mail, Phone, Calendar, Upload, FileText, AtSign, Loader2, PlayCircle, Clock, HelpCircle, ListChecks, History, CheckCircle2, XCircle, AlertCircle } from "lucide-react"
import { format } from "date-fns"
import { ru as localeRu, enUS as localeEn } from "date-fns/locale"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { toast } from "sonner"
import { useQuery } from "@tanstack/react-query"
import { useRouter } from "next/navigation"
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog"
import { ExternalVacancyListResponse, CandidateApplication } from "@/types/api"
import { apiFetch } from "@/lib/api"
import { Briefcase } from "lucide-react"
import { useTranslation } from "@/lib/i18n-context"
import { useQueryClient } from "@tanstack/react-query"

interface CandidateProfileCardProps {
    candidate: {
        id: string
        name: string
        email: string
        phone?: string
        telegram_id?: number
        dob?: string
        created_at: string
        cv_url?: string
        profile_data?: any
    }
    onCvUpdated?: (newCvUrl: string) => void
    isPublicView?: boolean
}

export function CandidateProfileCard({ candidate, onCvUpdated, isPublicView = false }: CandidateProfileCardProps) {
    const { t, language } = useTranslation()
    const [isUploading, setIsUploading] = useState(false)
    const fileInputRef = useRef<HTMLInputElement>(null)
    const router = useRouter()
    const queryClient = useQueryClient()

    // Fetch vacancies to resolve titles
    const { data: vacancyData } = useQuery({
        queryKey: ['external-vacancies'],
        queryFn: () => apiFetch<ExternalVacancyListResponse>('/api/external-vacancies'),
    });

    // Fetch candidate applications
    const { data: applications } = useQuery({
        queryKey: ['candidate-applications', candidate.id],
        queryFn: () => apiFetch<CandidateApplication[]>(`/api/candidate/${candidate.id}/applications`),
        enabled: !!candidate.id
    });

    const getVacancyTitle = (id: number) => {
        return vacancyData?.vacancies.find(v => v.id === id)?.title || `Vacancy #${id}`;
    };

    interface HistoryItem {
        event_type: string;
        title: string;
        description: string | null;
        timestamp: string;
        status: string | null;
        metadata: any;
    }

    const { data: history, isLoading: isHistoryLoading } = useQuery<HistoryItem[]>({
        queryKey: ['candidate-history', candidate.id],
        queryFn: () => apiFetch<HistoryItem[]>(`/api/candidate/${candidate.id}/history`),
        enabled: !!candidate.id,
        refetchInterval: 30000, // Refresh every 30 seconds
    });

    const getHistoryIcon = (eventType: string, status: string | null) => {
        switch (eventType) {
            case 'registration':
                return <User className="h-4 w-4 text-green-500" />;
            case 'application':
                return <Briefcase className="h-4 w-4 text-blue-500" />;
            case 'profile_update':
                return <FileText className="h-4 w-4 text-purple-500" />;
            case 'test_attempt':
                if (status === 'Passed') return <CheckCircle2 className="h-4 w-4 text-green-500" />;
                if (status === 'Failed') return <XCircle className="h-4 w-4 text-red-500" />;
                if (status === 'Pending') return <Clock className="h-4 w-4 text-yellow-500" />;
                return <AlertCircle className="h-4 w-4 text-orange-500" />;
            default:
                return <History className="h-4 w-4 text-muted-foreground" />;
        }
    };

    const handleCvUpdate = async (file: File) => {
        setIsUploading(true)
        try {
            const formData = new FormData()
            formData.append("cv", file)

            const response = await fetch(`${process.env.NEXT_PUBLIC_API_URL || ""}/api/candidate/${candidate.id}/cv`, {
                method: "PATCH",
                body: formData,
            })

            if (!response.ok) {
                throw new Error("Failed to update CV")
            }

            const updated = await response.json()
            toast.success(t('candidate_profile.cv_updated'))
            onCvUpdated?.(updated.cv_url)
        } catch (error) {
            console.error(error)
            toast.error(t('candidate_profile.cv_update_error'))
        } finally {
            setIsUploading(false)
        }
    }

    const { data: pendingInvites } = useQuery({
        queryKey: ["candidate-invites", candidate.email],
        queryFn: async () => {
            if (!isPublicView || !candidate.email) return []
            const apiUrl = process.env.NEXT_PUBLIC_API_URL || ""
            const res = await fetch(`${apiUrl}/api/integration/test-attempts?candidate_email=${candidate.email}&status=pending`)
            if (!res.ok) return []
            const data = await res.json()
            return data.items || []
        },
        enabled: isPublicView && !!candidate.email,
        refetchInterval: 15000, // Task 9: Poll every 15 seconds for real-time updates
    })

    // Task 16: Query for in-progress attempts (especially presentation tests)
    const { data: inProgressAttempts } = useQuery({
        queryKey: ["candidate-in-progress", candidate.email],
        queryFn: async () => {
            if (!isPublicView || !candidate.email) return []
            const apiUrl = process.env.NEXT_PUBLIC_API_URL || ""
            const res = await fetch(`${apiUrl}/api/integration/test-attempts?candidate_email=${candidate.email}&status=in_progress`)
            if (!res.ok) return []
            const data = await res.json()
            return data.items || []
        },
        enabled: isPublicView && !!candidate.email,
        refetchInterval: 15000, // Poll for updates
    })

    const hasPendingTest = pendingInvites && pendingInvites.length > 0
    const latestTest = hasPendingTest ? pendingInvites[0] : null

    // Task 16: Find any in-progress presentation test that hasn't expired
    const inProgressPresentationTest = inProgressAttempts?.find((attempt: any) => {
        if (!attempt.expires_at) return false
        const expiresAt = new Date(attempt.expires_at)
        const now = new Date()
        return expiresAt > now // Only show if not expired
    })

    // Fetch details for the pending test to show in the modal
    const { data: testDetails, isLoading: isTestDetailsLoading } = useQuery({
        queryKey: ['test-details', latestTest?.test_id],
        queryFn: () => apiFetch<any>(`/api/integration/tests/${latestTest.test_id}`),
        enabled: !!latestTest?.test_id,
    });

    // Task 16: Fetch details for in-progress presentation test
    const { data: inProgressTestDetails } = useQuery({
        queryKey: ['test-details-in-progress', inProgressPresentationTest?.test_id],
        queryFn: () => apiFetch<any>(`/api/integration/tests/${inProgressPresentationTest.test_id}`),
        enabled: !!inProgressPresentationTest?.test_id,
    });

    // Task 16: Calculate time remaining for in-progress test
    const getTimeRemaining = (expiresAt: string) => {
        const expires = new Date(expiresAt)
        const now = new Date()
        const diffMs = expires.getTime() - now.getTime()
        if (diffMs <= 0) return null
        const hours = Math.floor(diffMs / (1000 * 60 * 60))
        const minutes = Math.floor((diffMs % (1000 * 60 * 60)) / (1000 * 60))
        if (hours > 0) {
            return `${hours}h ${minutes}m`
        }
        return `${minutes}m`
    }

    return (
        <div className="space-y-6">
            {/* Task 16: Continue Test button for in-progress presentation tests */}
            {isPublicView && inProgressPresentationTest && inProgressTestDetails?.test_type === 'presentation' && getTimeRemaining(inProgressPresentationTest.expires_at) && (
                <Card className="border-blue-500/50 bg-blue-500/5 dark:bg-blue-500/10 shadow-lg animate-in fade-in slide-in-from-top-4">
                    <CardHeader className="pb-2">
                        <CardTitle className="text-lg flex items-center gap-2 text-blue-700 dark:text-blue-300">
                            <Clock className="h-5 w-5" />
                            {t('test.continue_presentation') || 'Continue Presentation'}
                        </CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="flex flex-col gap-4">
                            <div className="flex items-center justify-between">
                                <p className="text-sm text-foreground/80">
                                    {inProgressTestDetails?.title}
                                </p>
                                <Badge variant="outline" className="text-blue-600 border-blue-500">
                                    <Clock className="h-3 w-3 mr-1" />
                                    {getTimeRemaining(inProgressPresentationTest.expires_at)}
                                </Badge>
                            </div>
                            <Button
                                className="w-full bg-blue-600 hover:bg-blue-700 text-white gap-2"
                                onClick={() => {
                                    if (inProgressPresentationTest.access_token) {
                                        router.push(`/test/${inProgressPresentationTest.access_token}`)
                                    }
                                }}
                            >
                                {t('test.continue_presentation') || 'Continue Presentation'}
                                <PlayCircle className="h-4 w-4" />
                            </Button>
                        </div>
                    </CardContent>
                </Card>
            )}

            {isPublicView && hasPendingTest && latestTest && (
                <Card className="border-green-500/50 bg-green-500/5 dark:bg-green-500/10 shadow-lg animate-in fade-in slide-in-from-top-4">
                    <CardHeader className="pb-2">
                        <CardTitle className="text-lg flex items-center gap-2 text-green-700 dark:text-green-300">
                            <PlayCircle className="h-5 w-5" />
                            {t('candidate_profile.pending_invite')}
                        </CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="flex flex-col gap-4">
                            <p className="text-sm text-foreground/80">
                                {t('candidate_profile.invite_desc')}
                            </p>

                            <Dialog>
                                <DialogTrigger asChild>
                                    <Button className="w-full bg-green-600 hover:bg-green-700 text-white gap-2">
                                        {t('candidate_profile.start_test_btn')}
                                        <PlayCircle className="h-4 w-4" />
                                    </Button>
                                </DialogTrigger>
                                <DialogContent className="sm:max-w-md">
                                    <DialogHeader>
                                        <DialogTitle className="flex items-center gap-2 text-xl">
                                            <FileText className="h-5 w-5 text-primary" />
                                            {testDetails?.title ? (
                                                testDetails.test_type === 'presentation' ? `${testDetails.title} (${t('dashboard.attempts.presentation.title').toLowerCase()})` : testDetails.title
                                            ) : t('common.loading')}
                                        </DialogTitle>
                                    </DialogHeader>

                                    {isTestDetailsLoading ? (
                                        <div className="flex justify-center py-8">
                                            <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
                                        </div>
                                    ) : (
                                        <div className="space-y-4">
                                            <div className="p-4 rounded-lg bg-muted/50 border">
                                                <p className="text-sm text-muted-foreground italic">
                                                    {testDetails?.description || t('test.no_description')}
                                                </p>
                                            </div>

                                            <div className="grid grid-cols-2 gap-4">
                                                <div className="flex items-center gap-3 p-3 rounded-lg bg-secondary/20 border">
                                                    <Clock className="h-5 w-5 text-primary" />
                                                    <div>
                                                        <p className="text-xs text-muted-foreground uppercase tracking-wider">{t('test.duration')}</p>
                                                        <p className="text-lg font-bold">
                                                            {testDetails?.test_type === 'presentation'
                                                                ? `${(testDetails?.duration_minutes / 60).toFixed(0)} ${t('dashboard.attempts.presentation.hours')}`
                                                                : `${testDetails?.duration_minutes} ${t('test.duration_unit')}`
                                                            }
                                                        </p>
                                                    </div>
                                                </div>
                                                <div className="flex items-center gap-3 p-3 rounded-lg bg-secondary/20 border">
                                                    {testDetails?.test_type === 'presentation' ? (
                                                        <ListChecks className="h-5 w-5 text-primary" />
                                                    ) : (
                                                        <HelpCircle className="h-5 w-5 text-primary" />
                                                    )}
                                                    <div>
                                                        <p className="text-xs text-muted-foreground uppercase tracking-wider">
                                                            {testDetails?.test_type === 'presentation'
                                                                ? t('dashboard.attempts.presentation.themes')
                                                                : t('test.questions')
                                                            }
                                                        </p>
                                                        <p className="text-lg font-bold">
                                                            {testDetails?.test_type === 'presentation'
                                                                ? testDetails?.presentation_themes?.length || 0
                                                                : testDetails?.questions?.length || 0
                                                            }
                                                        </p>
                                                    </div>
                                                </div>
                                            </div>

                                            {/* Task 15: Instructions section */}
                                            <div className="p-4 rounded-lg bg-amber-50/50 dark:bg-amber-900/20 border border-amber-200/50 dark:border-amber-800/50">
                                                <h4 className="text-sm font-semibold text-amber-800 dark:text-amber-300 mb-2 flex items-center gap-2">
                                                    <FileText className="h-4 w-4" />
                                                    {t('test.instructions')}
                                                </h4>
                                                <p className="text-sm text-amber-700/80 dark:text-amber-300/80">
                                                    {testDetails?.test_type === 'presentation'
                                                        ? (testDetails?.instructions || t('candidate_profile.presentation_instructions') || 'Review the presentation themes carefully. You can work offline and return to submit your work before the deadline. Make sure your presentation link is public or upload the file directly.')
                                                        : (testDetails?.instructions || t('test.default_instructions'))
                                                    }
                                                </p>
                                            </div>

                                            <div className="pt-4">
                                                <Button
                                                    className="w-full h-12 text-lg gap-2 shadow-md hover:shadow-lg transition-all"
                                                    onClick={() => {
                                                        if (latestTest.access_token) {
                                                            router.push(`/test/${latestTest.access_token}?autostart=true`)
                                                        } else {
                                                            toast.error(t('test.error_load'))
                                                        }
                                                    }}
                                                >
                                                    {t('test.start_btn')}
                                                    <PlayCircle className="h-5 w-5" />
                                                </Button>
                                                <p className="text-xs text-center text-muted-foreground mt-3">
                                                    {t('test.exit_note')}
                                                </p>
                                            </div>
                                        </div>
                                    )}
                                </DialogContent>
                            </Dialog>
                        </div>
                    </CardContent>
                </Card>
            )}

            <Card className="overflow-hidden premium-hover transition-all duration-300">
                <CardHeader className="pb-3 flex flex-row items-center justify-between space-y-0 bg-gradient-to-r from-primary/10 to-primary/5">
                    <div className="flex items-center gap-3">
                        <div className="h-12 w-12 rounded-full bg-primary/20 flex items-center justify-center ring-2 ring-primary/30">
                            <User className="h-7 w-7 text-primary" />
                        </div>
                        <div>
                            <CardTitle className="text-xl leading-none">{candidate.name}</CardTitle>
                            {candidate.telegram_id && (
                                <div className="flex items-center gap-1 mt-1 text-xs text-muted-foreground">
                                    <AtSign className="h-3 w-3" />
                                    <span>{t('candidate_profile.telegram_id')}: {candidate.telegram_id}</span>
                                </div>
                            )}
                        </div>
                    </div>
                    {candidate.telegram_id && (
                        <Badge variant="secondary" className="font-normal bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300">
                            <span className="flex items-center gap-1">
                                {t('candidate_profile.connected')}
                            </span>
                        </Badge>
                    )}
                </CardHeader>
                <CardContent className="pt-4 space-y-4">
                    <div className="grid gap-3 text-sm">
                        <div className="flex items-center gap-3 p-2 rounded-lg bg-muted/50">
                            <Mail className="h-4 w-4 text-primary" />
                            <span className="text-foreground font-medium">{candidate.email}</span>
                        </div>
                        <div className="flex items-center gap-3 p-2 rounded-lg bg-muted/50">
                            <Phone className="h-4 w-4 text-primary" />
                            <span className="text-foreground">{candidate.phone || t('candidate_profile.not_provided')}</span>
                        </div>
                        {candidate.dob && (
                            <div className="flex items-center gap-3 p-2 rounded-lg bg-muted/50">
                                <Cake className="h-4 w-4 text-primary" />
                                <span className="text-foreground">
                                    {format(new Date(candidate.dob), "LLLL d, yyyy", { locale: language === 'ru' ? localeRu : localeEn })}
                                </span>
                            </div>
                        )}
                        <div className="flex items-center gap-3 p-2 rounded-lg bg-muted/50">
                            <Calendar className="h-4 w-4 text-primary" />
                            <span className="text-muted-foreground">
                                {t('candidate_profile.registered')}: {format(new Date(candidate.created_at), "MMM d, yyyy", { locale: language === 'ru' ? localeRu : localeEn })}
                            </span>
                        </div>

                        <Dialog>
                            <DialogTrigger asChild>
                                <Button variant="outline" className="w-full justify-start gap-3 h-auto p-2 px-3 font-normal hover:bg-muted/50">
                                    <Briefcase className="h-4 w-4 text-primary" />
                                    <div className="flex flex-col items-start">
                                        <span className="text-foreground font-medium">{t('candidate_profile.vacancies_applied')} ({applications?.length || 0})</span>
                                        <span className="text-xs text-muted-foreground">{t('candidate_profile.click_details')}</span>
                                    </div>
                                </Button>
                            </DialogTrigger>
                            <DialogContent>
                                <DialogHeader>
                                    <DialogTitle>{t('candidate_profile.applied_vacancies')}</DialogTitle>
                                </DialogHeader>
                                <div className="space-y-4 max-h-[60vh] overflow-y-auto">
                                    {applications?.length === 0 ? (
                                        <p className="text-center text-muted-foreground py-4">{t('candidate_profile.no_applications')}</p>
                                    ) : (
                                        applications?.map((app) => (
                                            <div key={app.id} className="border rounded-lg p-3 space-y-1">
                                                <div className="font-medium" dangerouslySetInnerHTML={{ __html: getVacancyTitle(app.vacancy_id) }} />
                                                <div className="text-xs text-muted-foreground">
                                                    {t('candidate_profile.applied_on')} {format(new Date(app.created_at), "PPP", { locale: language === 'ru' ? localeRu : localeEn })}
                                                </div>
                                            </div>
                                        ))
                                    )}
                                </div>
                            </DialogContent>
                        </Dialog>

                        {/* Task 13: History Button and Modal */}
                        <Dialog>
                            <DialogTrigger asChild>
                                <Button variant="outline" className="w-full justify-start gap-3 h-auto p-2 px-3 font-normal hover:bg-muted/50 mt-2">
                                    <History className="h-4 w-4 text-primary" />
                                    <div className="flex flex-col items-start">
                                        <span className="text-foreground font-medium">{t('candidate_profile.history') || 'Activity History'}</span>
                                        <span className="text-xs text-muted-foreground">{t('candidate_profile.history_desc') || 'View all activities'}</span>
                                    </div>
                                </Button>
                            </DialogTrigger>
                            <DialogContent className="max-w-lg">
                                <DialogHeader>
                                    <DialogTitle className="flex items-center gap-2">
                                        <History className="h-5 w-5 text-primary" />
                                        {t('candidate_profile.history_title') || 'Activity History'}
                                    </DialogTitle>
                                </DialogHeader>
                                <div className="space-y-3 max-h-[60vh] overflow-y-auto">
                                    {isHistoryLoading ? (
                                        <div className="flex justify-center py-8">
                                            <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
                                        </div>
                                    ) : history?.length === 0 ? (
                                        <p className="text-center text-muted-foreground py-4">{t('candidate_profile.no_history') || 'No activity yet'}</p>
                                    ) : (
                                        history?.map((item, index) => (
                                            <div key={index} className="flex gap-3 border-l-2 border-primary/20 pl-3 py-2 hover:bg-muted/50 rounded-r-lg transition-colors">
                                                <div className="flex-shrink-0 mt-1">
                                                    {getHistoryIcon(item.event_type, item.status)}
                                                </div>
                                                <div className="flex-1 min-w-0">
                                                    <div className="flex items-center justify-between gap-2">
                                                        <span className="font-medium text-sm truncate">
                                                            {item.event_type === 'registration' && (t('candidate_profile.event_registered') || 'Registered')}
                                                            {item.event_type === 'application' && (t('candidate_profile.event_applied') || 'Applied for vacancy')}
                                                            {item.event_type === 'profile_update' && (t('candidate_profile.event_update') || 'Profile Updated')}
                                                            {item.event_type === 'test_attempt' && (t('candidate_profile.event_test') || 'Test attempt')}
                                                        </span>
                                                        {item.status && (
                                                            <Badge variant="outline" className="text-xs flex-shrink-0">
                                                                {(() => {
                                                                    const s = item.status;
                                                                    if (s === 'Passed') return t('dashboard.attempts.labels.passed');
                                                                    if (s === 'Failed') return t('dashboard.attempts.labels.failed');
                                                                    if (s === 'Pending') return t('dashboard.attempts.statuses.pending');
                                                                    if (s === 'In Progress') return t('dashboard.attempts.statuses.in_progress');
                                                                    if (s === 'Timed Out') return t('dashboard.attempts.statuses.timeout');
                                                                    if (s === 'Left Page') return t('dashboard.attempts.statuses.escaped');
                                                                    if (s === 'Needs Review') return t('dashboard.attempts.statuses.needs_review');
                                                                    if (s === 'submitted') return t('dashboard.attempts.card.submitted');
                                                                    if (s === 'completed') return t('dashboard.attempts.statuses.completed');
                                                                    return s;
                                                                })()}
                                                            </Badge>
                                                        )}
                                                    </div>
                                                    {(() => {
                                                        if (!item.description) return null;

                                                        // Score
                                                        if (item.event_type === 'test_attempt') {
                                                            if (item.metadata?.percentage !== undefined) {
                                                                return <p className="text-xs text-muted-foreground mt-0.5">{t('dashboard.attempts.labels.score')}: {item.metadata.percentage}%</p>;
                                                            }
                                                            if (item.description.startsWith("Score:")) {
                                                                return <p className="text-xs text-muted-foreground mt-0.5">{t('dashboard.attempts.labels.score')}: {item.description.split(': ')[1]}</p>;
                                                            }
                                                        }

                                                        // Vacancy ID
                                                        if (item.description.startsWith("Vacancy ID:")) {
                                                            const id = item.description.split(': ')[1].trim();
                                                            return <p className="text-xs text-muted-foreground mt-0.5">{t('common.vacancy')} ID: {id}</p>;
                                                        }

                                                        return <p className="text-xs text-muted-foreground mt-0.5">{item.description}</p>;
                                                    })()}
                                                    <p className="text-xs text-muted-foreground/70 mt-1">
                                                        {format(new Date(item.timestamp), "PPp", { locale: language === 'ru' ? localeRu : localeEn })}
                                                    </p>
                                                </div>
                                            </div>
                                        ))
                                    )}
                                </div>
                            </DialogContent>
                        </Dialog>
                    </div>

                    <div className="pt-2">
                        <input
                            ref={fileInputRef}
                            type="file"
                            className="hidden"
                            accept=".pdf,.docx,.doc,.png,.jpg,.jpeg,.webp"
                            onChange={(e) => {
                                const files = e.target.files
                                if (files && files[0]) {
                                    handleCvUpdate(files[0])
                                }
                            }}
                        />
                        <Button
                            variant="outline"
                            size="sm"
                            className="w-full gap-2"
                            disabled={isUploading}
                            onClick={() => fileInputRef.current?.click()}
                        >
                            {isUploading ? (
                                <>
                                    <Loader2 className="h-4 w-4 animate-spin" />
                                    {t('candidate_profile.uploading')}
                                </>
                            ) : candidate.cv_url ? (
                                <>
                                    <Upload className="h-4 w-4" />
                                    {t('candidate_profile.update_cv')}
                                </>
                            ) : (
                                <>
                                    <FileText className="h-4 w-4" />
                                    {t('candidate_profile.upload_cv')}
                                </>
                            )}
                        </Button>
                        {candidate.cv_url && (
                            <p className="text-xs text-muted-foreground text-center mt-2">
                                {t('candidate_profile.current_cv')}: {candidate.cv_url.split('/').pop()?.substring(37) || "CV uploaded"}
                            </p>
                        )}
                    </div>
                </CardContent>
            </Card>
        </div>
    )
}

function Cake(props: any) {
    return (
        <svg
            {...props}
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
        >
            <path d="M20 21v-8a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8" />
            <path d="M4 16s.5-1 2-1 2.5 1 4 1 2.5-1 4-1 2.5 1 4 1 2-1 2-1" />
            <path d="M2 21h20" />
            <path d="M7 8v2" />
            <path d="M12 8v2" />
            <path d="M17 8v2" />
            <path d="M7 4h.01" />
            <path d="M12 4h.01" />
            <path d="M17 4h.01" />
        </svg>
    )
}
