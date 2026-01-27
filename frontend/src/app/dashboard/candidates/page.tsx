"use client"
import { cn } from "@/lib/utils"

import { useState, useMemo, useEffect, useRef, Suspense } from "react"
import { motion, AnimatePresence } from "framer-motion"
import { ru as localeRu, enUS as localeEn } from "date-fns/locale"

import { useQuery } from "@tanstack/react-query"
import {
    Plus, Search, User, Mail, Phone, Calendar, Briefcase, FileText,
    Download, Sparkles, Binary, Loader2, ChevronRight, LayoutGrid,
    List, History, CheckCircle2, Clock, XCircle, Send, MessageSquare,
    AlertCircle, Cake
} from "lucide-react"
import { format } from "date-fns"
import Link from "next/link"
import { useSearchParams } from "next/navigation"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog"
import { Textarea } from "@/components/ui/textarea"
import { CandidateApplication, ExternalVacancyListResponse } from "@/types/api"
import { apiFetch } from "@/lib/api"
import { useTranslation } from "@/lib/i18n-context"
import { useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

async function fetchCandidates() {
    const response = await fetch(`${process.env.NEXT_PUBLIC_API_URL || ""}/api/integration/candidates`)
    if (!response.ok) throw new Error("Failed to fetch candidates")
    return response.json()
}

function CandidatesPageContent() {
    const { t } = useTranslation()
    const searchParams = useSearchParams()
    const highlightId = searchParams.get('highlight')
    const [highlightedId, setHighlightedId] = useState<string | null>(null)
    const cardRefs = useRef<Map<string, HTMLDivElement>>(new Map())

    const [searchQuery, setSearchQuery] = useState("")
    const [layout, setLayoutState] = useState<'grid' | 'list'>('grid')
    const [viewVacanciesCandidate, setViewVacanciesCandidate] = useState<any>(null);
    const [viewHistoryCandidate, setViewHistoryCandidate] = useState<any>(null);
    const [messageCandidate, setMessageCandidate] = useState<any>(null);
    const [messageText, setMessageText] = useState("");
    const [isSendingMessage, setIsSendingMessage] = useState(false);
    const [isBlurring, setIsBlurring] = useState(false)
    const [isAnalyzingId, setIsAnalyzingId] = useState<string | null>(null)
    const queryClient = useQueryClient()
    const { language } = useTranslation()

    useEffect(() => {
        const saved = localStorage.getItem('candidates-layout')
        if (saved === 'grid' || saved === 'list') {
            setLayoutState(saved)
        }
    }, [])

    const setLayout = (l: 'grid' | 'list') => {
        setLayoutState(l)
        localStorage.setItem('candidates-layout', l)
    }
    const dateLocale = language === 'ru' ? localeRu : localeEn


    // Fetch vacancies to resolve titles
    const { data: vacancyData } = useQuery({
        queryKey: ['external-vacancies'],
        queryFn: () => apiFetch<ExternalVacancyListResponse>('/api/integration/external-vacancies'),
    });

    // Fetch candidate applications when a candidate is selected
    const { data: applications, isLoading: applicationsLoading } = useQuery({
        queryKey: ['candidate-applications', viewVacanciesCandidate?.id],
        queryFn: () => apiFetch<CandidateApplication[]>(`/api/candidate/${viewVacanciesCandidate?.id}/applications`),
        enabled: !!viewVacanciesCandidate,
    });

    // Fetch history
    const { data: history, isLoading: isHistoryLoading } = useQuery({
        queryKey: ['candidate-history', viewHistoryCandidate?.id],
        queryFn: () => apiFetch<any[]>(`/api/candidate/${viewHistoryCandidate?.id}/history`),
        enabled: !!viewHistoryCandidate,
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

    const getVacancyTitle = (id: number) => {
        return vacancyData?.vacancies.find(v => v.id === id)?.title || `Vacancy #${id}`;
    };

    const handleAiAnalyze = async (candidateId: string) => {
        try {
            setIsAnalyzingId(candidateId);
            await apiFetch(`/api/integration/analyze-suitability/${candidateId}`, { method: 'POST' });
            queryClient.invalidateQueries({ queryKey: ["candidates"] });
            toast.success(t('candidate_profile.ai_success') || "AI Analysis updated");
        } catch (e) {
            toast.error(t('candidate_profile.ai_error') || "Failed to re-run AI analysis");
        } finally {
            setIsAnalyzingId(null);
        }
    };

    const handleSendMessage = async () => {
        if (!messageCandidate || !messageText.trim()) return;

        try {
            setIsSendingMessage(true);
            await apiFetch(`/api/integration/messages`, {
                method: 'POST',
                body: JSON.stringify({
                    candidate_id: messageCandidate.id,
                    text: messageText
                })
            });
            toast.success(t('common.success') || "Message sent successfully");
            setMessageCandidate(null);
            setMessageText("");
        } catch (e) {
            console.error(e);
            toast.error(t('common.error') || "Failed to send message");
        } finally {
            setIsSendingMessage(false);
        }
    };

    const AiAssessmentDialog = ({ candidate, trigger }: { candidate: any, trigger: React.ReactNode }) => (
        <Dialog>
            <DialogTrigger asChild>
                {trigger}
            </DialogTrigger>
            <DialogContent className="sm:max-w-md">
                <DialogHeader>
                    <DialogTitle className="flex items-center gap-2">
                        <Sparkles className="h-5 w-5 text-primary" />
                        {t('candidate_profile.ai_assessment_title')}
                    </DialogTitle>
                </DialogHeader>
                <div className="space-y-6 pt-2">
                    {isAnalyzingId === candidate.id ? (
                        <div className="flex flex-col items-center justify-center py-12 gap-4">
                            <Loader2 className="h-12 w-12 animate-spin text-primary" />
                            <p className="text-sm font-medium animate-pulse text-muted-foreground">{t('common.analyzing')}</p>
                        </div>
                    ) : candidate.ai_rating !== undefined ? (
                        <div className="space-y-6">
                            <div className="p-5 rounded-2xl bg-primary/[0.03] border border-primary/10 relative overflow-hidden group">
                                <div className="absolute -top-6 -right-6 h-24 w-24 bg-primary/5 rounded-full blur-2xl group-hover:bg-primary/10 transition-colors" />
                                <div className="flex items-center justify-between mb-3 relative z-10">
                                    <div className="flex items-center gap-2">
                                        <Sparkles className="h-4 w-4 text-primary" />
                                        <span className="text-xs font-bold uppercase tracking-wider text-primary/70">{t('candidate_profile.ai_suitability_title')}</span>
                                    </div>
                                    <Badge variant="outline" className="h-6 px-2 text-xs border-primary/30 text-primary bg-primary/10">
                                        {candidate.ai_rating}%
                                    </Badge>
                                </div>
                                <p className="text-sm leading-relaxed italic text-foreground relative z-10">
                                    &ldquo;{candidate.ai_comment}&rdquo;
                                </p>
                            </div>
                            <div className="flex justify-center">
                                <Button
                                    onClick={() => handleAiAnalyze(candidate.id)}
                                    variant="outline"
                                    className="gap-2 border-primary/20 hover:bg-primary/5"
                                >
                                    <Binary className="h-4 w-4" />
                                    {t('common.regenerate')}
                                </Button>
                            </div>
                        </div>
                    ) : (
                        <div className="flex flex-col items-center justify-center py-10 px-6 gap-5 border-2 border-dashed rounded-2xl bg-muted/30">
                            <div className="h-16 w-16 rounded-full bg-background flex items-center justify-center shadow-sm border">
                                <Sparkles className="h-8 w-8 text-muted-foreground/40" />
                            </div>
                            <div className="text-center space-y-1">
                                <p className="text-sm font-semibold">{t('candidate_profile.no_ai_data')}</p>
                                <p className="text-xs text-muted-foreground leading-relaxed max-w-[240px] mx-auto">
                                    {t('candidate_profile.ai_desc')}
                                </p>
                            </div>
                            <Button
                                onClick={() => handleAiAnalyze(candidate.id)}
                                className="gap-2 shadow-md hover:shadow-lg transition-all"
                            >
                                <Sparkles className="h-4 w-4" />
                                {t('common.generate')}
                            </Button>
                        </div>
                    )}
                </div>
            </DialogContent>
        </Dialog>
    );

    const { data: candidates, isLoading } = useQuery({
        queryKey: ["candidates"],
        queryFn: fetchCandidates,
    })

    // Handle highlight and scroll
    useEffect(() => {
        if (highlightId && candidates) {
            // Larger delay to ensure cards are fully rendered and layout is stable
            const timer = setTimeout(() => {
                const cardEl = cardRefs.current.get(highlightId)
                if (cardEl) {
                    cardEl.scrollIntoView({ behavior: 'smooth', block: 'center' })

                    // Wait for scroll to finish before highlighting for better smoothness
                    setTimeout(() => {
                        setHighlightedId(highlightId)
                        setIsBlurring(true)

                        // Stop blurring after 1 second as requested
                        setTimeout(() => setIsBlurring(false), 1000)

                        // Remove highlight after 5 seconds
                        setTimeout(() => {
                            setHighlightedId(null)
                        }, 5000)
                    }, 600)
                }
            }, 500)
            return () => clearTimeout(timer)
        }
    }, [highlightId, candidates])

    // Robust search - matches name, email, phone, and telegram ID
    const filteredCandidates = useMemo(() => {
        if (!candidates || !searchQuery.trim()) return candidates || []

        const query = searchQuery.toLowerCase().trim()

        return candidates.filter((candidate: any) => {
            const name = (candidate.name || "").toLowerCase()
            const email = (candidate.email || "").toLowerCase()
            const phone = (candidate.phone || "").toLowerCase().replace(/\s+/g, "")
            const telegramId = String(candidate.telegram_id || "")
            const queryNoSpaces = query.replace(/\s+/g, "")

            return (
                name.includes(query) ||
                email.includes(query) ||
                phone.includes(queryNoSpaces) ||
                telegramId.includes(query)
            )
        })
    }, [candidates, searchQuery])

    if (isLoading) {
        return <div className="p-8 text-center text-muted-foreground">{t('common.loading')}</div>
    }

    return (
        <div className="space-y-6">
            <motion.div
                animate={isBlurring ? { filter: 'blur(4px)', opacity: 0.6, scale: 0.99 } : { filter: 'blur(0px)', opacity: 1, scale: 1 }}
                transition={{ duration: 0.5 }}
                className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between"
            >
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">{t('dashboard.candidates.title')}</h1>
                    <p className="text-muted-foreground">
                        {t('dashboard.candidates.subtitle')}
                    </p>
                </div>
                <div className="flex items-center gap-3">
                    <div className="flex items-center gap-1 bg-muted/50 p-1 rounded-lg border shadow-sm">
                        <Button
                            variant={layout === 'grid' ? 'secondary' : 'ghost'}
                            size="sm"
                            className={cn("h-8 px-3 gap-2 transition-all", layout === 'grid' && "bg-background shadow-sm")}
                            onClick={() => setLayout('grid')}
                        >
                            <LayoutGrid className="h-4 w-4" />
                            <span className="text-xs font-medium hidden sm:inline">{t('common.layout_grid')}</span>
                        </Button>
                        <Button
                            variant={layout === 'list' ? 'secondary' : 'ghost'}
                            size="sm"
                            className={cn("h-8 px-3 gap-2 transition-all", layout === 'list' && "bg-background shadow-sm")}
                            onClick={() => setLayout('list')}
                        >
                            <List className="h-4 w-4" />
                            <span className="text-xs font-medium hidden sm:inline">{t('common.layout_list')}</span>
                        </Button>
                    </div>
                    <div className="relative w-64">
                        <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
                        <Input
                            placeholder={t('dashboard.candidates.search_placeholder')}
                            className="pl-8 bg-muted/30 focus:bg-background transition-colors"
                            value={searchQuery}
                            onChange={(e) => setSearchQuery(e.target.value)}
                        />
                    </div>
                </div>
            </motion.div>

            <div className={cn(
                "grid gap-4 transition-all duration-300",
                layout === 'grid' ? "grid-cols-1 md:grid-cols-2 lg:grid-cols-3" : "grid-cols-1"
            )}>
                <AnimatePresence initial={false}>
                    {filteredCandidates?.map((candidate: any) => (
                        <motion.div
                            key={candidate.id}
                            initial={{ opacity: 0, y: 10 }}
                            animate={{
                                opacity: isBlurring ? 0.6 : 1,
                                scale: highlightedId === candidate.id ? [1.03, 1.08, 1.03] : 1,
                                filter: isBlurring ? 'blur(4px)' : 'blur(0px)',
                                backgroundColor: highlightedId === candidate.id ? "rgba(255, 255, 255, 0.15)" : "rgba(0, 0, 0, 0)",
                                borderColor: highlightedId === candidate.id ? "rgba(255, 255, 255, 0.5)" : "transparent",
                                boxShadow: highlightedId === candidate.id ? "0 0 30px rgba(255, 255, 255, 0.2)" : "none",
                                y: 0
                            }}
                            exit={{ opacity: 0, scale: 0.95 }}
                            transition={{
                                duration: 0.2,
                                ease: "easeOut"
                            }}
                            ref={(el) => { if (el) cardRefs.current.set(candidate.id, el) }}
                            className={cn(
                                "rounded-xl overflow-hidden premium-hover border-2 z-0 relative",
                                highlightedId === candidate.id
                                    ? "z-10 ring-4 ring-white/20"
                                    : "border-transparent",
                                layout === 'list' && "w-full"
                            )}
                        >
                            <Card className={cn(
                                "border-0 bg-card/40 backdrop-blur-md shadow-sm h-full transition-all duration-300",
                                layout === 'list' ? "p-2" : "p-0"
                            )}>
                                {layout === 'grid' ? (
                                    <div className="flex flex-col h-full">
                                        <CardHeader className="pb-3 flex flex-row items-center justify-between space-y-0 bg-muted/30">
                                            <div className="flex items-center gap-3">
                                                <div className="h-10 w-10 rounded-full bg-primary/10 flex items-center justify-center">
                                                    <User className="h-6 w-6 text-primary" />
                                                </div>
                                                <div>
                                                    <CardTitle className="text-lg leading-none">{candidate.name}</CardTitle>
                                                </div>
                                            </div>
                                            <div className="flex items-center gap-2">
                                                <Button variant="ghost" size="sm" className="h-8 w-8 p-0 rounded-full hover:bg-primary/10" onClick={() => setMessageCandidate(candidate)}>
                                                    <MessageSquare className="h-4 w-4 text-primary" />
                                                </Button>
                                                <AiAssessmentDialog
                                                    candidate={candidate}
                                                    trigger={
                                                        <Button variant="ghost" size="sm" className="h-8 w-8 p-0 rounded-full hover:bg-primary/10 group/ai">
                                                            <Sparkles className={cn("h-4 w-4 text-primary transition-transform group-hover/ai:scale-110", isAnalyzingId === candidate.id && "animate-pulse")} />
                                                        </Button>
                                                    }
                                                />
                                            </div>
                                        </CardHeader>
                                        <CardContent className="pt-4 space-y-4 flex-1">
                                            <div className="grid gap-2 text-sm">
                                                <div className="flex items-center gap-2 text-muted-foreground">
                                                    <Mail className="h-4 w-4 shrink-0" />
                                                    <span className="text-foreground truncate">{candidate.email}</span>
                                                </div>
                                                <div className="flex items-center gap-2 text-muted-foreground">
                                                    <Phone className="h-4 w-4 shrink-0" />
                                                    <span className="text-foreground">{candidate.phone || "—"}</span>
                                                </div>
                                                {candidate.dob && (
                                                    <div className="flex items-center gap-2 text-muted-foreground">
                                                        <Cake className="h-4 w-4 shrink-0" />
                                                        <span className="text-foreground">
                                                            {t('dashboard.candidates.dob')}: {format(new Date(candidate.dob), "dd.MM.yyyy")}
                                                        </span>
                                                    </div>
                                                )}
                                                <div className="flex items-center gap-2 text-muted-foreground">
                                                    <Calendar className="h-4 w-4 shrink-0" />
                                                    <span>
                                                        {t('dashboard.candidates.registered_at')}: {format(new Date(candidate.created_at), "dd.MM.yyyy")}
                                                    </span>
                                                </div>
                                            </div>

                                            <div className="pt-2 flex items-center gap-2">
                                                {candidate.cv_url ? (
                                                    <Button variant="outline" size="sm" className="flex-1 gap-2" asChild>
                                                        <a href={`/${candidate.cv_url?.replace(/^\.?\/?/, '')}`} target="_blank" rel="noopener noreferrer">
                                                            <Download className="h-4 w-4" />
                                                            {t('dashboard.candidates.cv_download')}
                                                        </a>
                                                    </Button>
                                                ) : (
                                                    <Button variant="outline" size="sm" className="flex-1 gap-2" disabled>
                                                        <FileText className="h-4 w-4" />
                                                        {t('dashboard.candidates.cv_missing')}
                                                    </Button>
                                                )}
                                                <Button variant="default" size="sm" className="flex-1 gap-2" asChild>
                                                    <Link href={`/dashboard/invites?candidate=${candidate.id}`}>
                                                        <Send className="h-4 w-4" />
                                                        {t('common.invite')}
                                                    </Link>
                                                </Button>
                                            </div>


                                            <div className="grid grid-cols-2 gap-2">
                                                <Button
                                                    variant="secondary"
                                                    size="sm"
                                                    className="w-full gap-2 text-xs h-8"
                                                    onClick={() => setViewVacanciesCandidate(candidate)}
                                                >
                                                    <Briefcase className="h-3.5 w-3.5" />
                                                    {t('dashboard.candidates.applied_vacancies')}
                                                </Button>
                                                <Button
                                                    variant="outline"
                                                    size="sm"
                                                    className="w-full gap-2 text-xs h-8"
                                                    onClick={() => setViewHistoryCandidate(candidate)}
                                                >
                                                    <History className="h-3.5 w-3.5" />
                                                    {t('candidate_profile.history')}
                                                </Button>
                                            </div>
                                        </CardContent>
                                    </div>
                                ) : (
                                    <div className="flex flex-col md:flex-row items-center gap-6 p-4 w-full">
                                        <div className="flex items-center gap-5 w-full md:w-[30%] lg:w-[25%] relative group">
                                            <div className="h-12 w-12 rounded-full bg-primary/10 flex items-center justify-center shrink-0 shadow-inner">
                                                <User className="h-7 w-7 text-primary" />
                                            </div>
                                            <div className="min-w-0 flex-1 flex items-center gap-3">
                                                <CardTitle className="text-xl truncate group-hover:text-primary transition-colors cursor-default">{candidate.name}</CardTitle>
                                            </div>

                                            {/* List View AI Button Top Right of header area */}
                                            <div className="absolute top-0 right-0">
                                                <AiAssessmentDialog
                                                    candidate={candidate}
                                                    trigger={
                                                        <Button variant="ghost" size="sm" className="h-7 w-7 p-0 rounded-lg hover:bg-primary/10 group/ai">
                                                            <Sparkles className={cn("h-4 w-4 text-primary transition-transform group-hover/ai:scale-110", isAnalyzingId === candidate.id && "animate-pulse")} />
                                                        </Button>
                                                    }
                                                />
                                            </div>
                                        </div>

                                        <div className="grid grid-cols-2 lg:grid-cols-4 gap-y-3 gap-x-8 text-sm flex-1 w-full md:border-l px-2 md:px-6 border-muted-foreground/10 py-1">
                                            <div className="flex flex-col space-y-0.5">
                                                <span className="text-[10px] text-muted-foreground/60 uppercase tracking-wider font-semibold">Email</span>
                                                <div className="flex items-center gap-2">
                                                    <Mail className="h-3.5 w-3.5 text-primary/70 shrink-0" />
                                                    <span className="text-foreground truncate max-w-[150px]">{candidate.email}</span>
                                                </div>
                                            </div>
                                            <div className="flex flex-col space-y-0.5">
                                                <span className="text-[10px] text-muted-foreground/60 uppercase tracking-wider font-semibold">{t('registration.phone') || 'Phone'}</span>
                                                <div className="flex items-center gap-2">
                                                    <Phone className="h-3.5 w-3.5 text-primary/70 shrink-0" />
                                                    <span className="text-foreground">{candidate.phone || "—"}</span>
                                                </div>
                                            </div>
                                            <div className="flex flex-col space-y-0.5">
                                                <span className="text-[10px] text-muted-foreground/60 uppercase tracking-wider font-semibold">{t('dashboard.candidates.registered_at')}</span>
                                                <div className="flex items-center gap-2">
                                                    <Calendar className="h-3.5 w-3.5 text-primary/70 shrink-0" />
                                                    <span className="text-foreground">{format(new Date(candidate.created_at), "dd.MM.yyyy")}</span>
                                                </div>
                                            </div>
                                            <div className="flex flex-col space-y-0.5">
                                                <span className="text-[10px] text-muted-foreground/60 uppercase tracking-wider font-semibold">{t('dashboard.candidates.dob')}</span>
                                                <div className="flex items-center gap-2">
                                                    <Cake className="h-3.5 w-3.5 text-primary/70 shrink-0" />
                                                    <span className="text-foreground">
                                                        {candidate.dob ? format(new Date(candidate.dob), "dd.MM.yyyy") : "—"}
                                                    </span>
                                                </div>
                                            </div>
                                        </div>

                                        <div className="flex items-center gap-2 w-full md:w-auto shrink-0 md:border-l pl-2 md:pl-6 border-muted-foreground/10">
                                            <div className="flex flex-wrap md:flex-nowrap gap-2 w-full md:w-auto">
                                                <div className="flex gap-2">
                                                    {candidate.cv_url ? (
                                                        <Button variant="outline" size="sm" className="h-9 px-3 gap-2 hover:bg-muted/80 shrink-0" asChild>
                                                            <a href={`/${candidate.cv_url?.replace(/^\.?\/?/, '')}`} target="_blank" rel="noopener noreferrer" title={t('dashboard.candidates.cv_download')}>
                                                                <Download className="h-4 w-4" />
                                                            </a>
                                                        </Button>
                                                    ) : (
                                                        <Button variant="outline" size="sm" className="h-9 px-3 gap-2 opacity-50 shrink-0" disabled>
                                                            <FileText className="h-4 w-4" />
                                                        </Button>
                                                    )}
                                                    <Button variant="outline" size="sm" className="h-9 px-3 gap-2 hover:bg-muted/80 shrink-0" onClick={() => setViewHistoryCandidate(candidate)} title={t('candidate_profile.history')}>
                                                        <History className="h-4 w-4" />
                                                    </Button>
                                                    <Button variant="ghost" size="sm" className="h-9 w-9 p-0 rounded-lg hover:bg-primary/10" onClick={() => setMessageCandidate(candidate)}>
                                                        <MessageSquare className="h-4 w-4 text-primary" />
                                                    </Button>
                                                </div>
                                                <div className="flex gap-2 flex-grow">
                                                    <Button variant="ghost" size="sm" className="h-9 px-3 gap-2 text-xs" onClick={() => setMessageCandidate(candidate)}>
                                                        <MessageSquare className="h-4 w-4" />
                                                        <span>{t('common.message') || "Message"}</span>
                                                    </Button>
                                                    <Button variant="secondary" size="sm" className="h-9 px-4 gap-2 text-xs flex-1 md:flex-none md:min-w-[120px]" onClick={() => setViewVacanciesCandidate(candidate)}>
                                                        <Briefcase className="h-4 w-4" />
                                                        <span className="hidden lg:inline">{t('dashboard.candidates.applied_vacancies')}</span>
                                                        <span className="lg:hidden">Vacancies</span>
                                                    </Button>
                                                    <Button variant="default" size="sm" className="h-9 px-4 gap-2 shadow-sm text-xs flex-1 md:flex-none md:min-w-[100px]" asChild>
                                                        <Link href={`/dashboard/invites?candidate=${candidate.id}`}>
                                                            <Send className="h-4 w-4" />
                                                            {t('common.invite')}
                                                        </Link>
                                                    </Button>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                )}
                            </Card>
                        </motion.div>
                    ))}
                </AnimatePresence>
            </div>

            {filteredCandidates?.length === 0 && searchQuery && (
                <div className="p-12 text-center border-2 border-dashed rounded-xl">
                    <div className="inline-flex h-12 w-12 items-center justify-center rounded-full bg-muted mb-4">
                        <Search className="h-6 w-6 text-muted-foreground" />
                    </div>
                    <h3 className="text-lg font-medium">{t('dashboard.vacancies.no_vacancies')}</h3>
                    <p className="text-muted-foreground max-w-sm mx-auto">
                        {t('dashboard.vacancies.no_vacancies')}
                    </p>
                    <Button
                        variant="outline"
                        className="mt-4"
                        onClick={() => setSearchQuery("")}
                    >
                        {t('common.back')}
                    </Button>
                </div>
            )}

            {candidates?.length === 0 && !searchQuery && (
                <div className="p-12 text-center border-2 border-dashed rounded-xl">
                    <div className="inline-flex h-12 w-12 items-center justify-center rounded-full bg-muted mb-4">
                        <User className="h-6 w-6 text-muted-foreground" />
                    </div>
                    <h3 className="text-lg font-medium">{t('dashboard.candidates.no_candidates')}</h3>
                    <p className="text-muted-foreground max-w-sm mx-auto">
                        {t('dashboard.candidates.no_candidates_desc')}
                    </p>
                </div>
            )}

            <Dialog open={!!messageCandidate} onOpenChange={(open) => !open && setMessageCandidate(null)}>
                <DialogContent className="sm:max-w-md">
                    <DialogHeader>
                        <DialogTitle className="flex items-center gap-2">
                            <MessageSquare className="h-5 w-5 text-primary" />
                            {t('dashboard.candidates.send_message') || "Send Message"}
                        </DialogTitle>
                        <DialogDescription>
                            {t('dashboard.candidates.send_message_desc')?.replace('{name}', messageCandidate?.name || '') || `The message will be delivered to ${messageCandidate?.name} via Telegram bot.`}
                        </DialogDescription>
                    </DialogHeader>
                    <div className="space-y-4 py-4">
                        <Textarea
                            placeholder={t('dashboard.candidates.message_placeholder') || "Enter your message..."}
                            value={messageText}
                            onChange={(e) => setMessageText(e.target.value)}
                            className="min-h-[120px] resize-none"
                        />
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setMessageCandidate(null)}>
                            {t('common.cancel')}
                        </Button>
                        <Button
                            onClick={handleSendMessage}
                            disabled={!messageText.trim() || isSendingMessage}
                            className="gap-2"
                        >
                            {isSendingMessage ? <Loader2 className="h-4 w-4 animate-spin" /> : <Send className="h-4 w-4" />}
                            {t('common.send')}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            <Dialog open={!!viewVacanciesCandidate} onOpenChange={(open) => !open && setViewVacanciesCandidate(null)}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>{t('dashboard.candidates.applied_vacancies')}: {viewVacanciesCandidate?.name}</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4 max-h-[60vh] overflow-y-auto">
                        {applicationsLoading ? (
                            <div className="text-center py-4">Loading applications...</div>
                        ) : applications?.length === 0 ? (
                            <p className="text-center text-muted-foreground py-4">No applications yet.</p>
                        ) : (
                            applications?.map((app) => (
                                <div key={app.id} className="border rounded-lg p-3 space-y-1">
                                    <div className="font-medium" dangerouslySetInnerHTML={{ __html: getVacancyTitle(app.vacancy_id) }} />
                                    <div className="text-xs text-muted-foreground">
                                        Applied on {format(new Date(app.created_at), "PPP")}
                                    </div>
                                </div>
                            ))
                        )}
                    </div>
                </DialogContent>
            </Dialog>

            <Dialog open={!!viewHistoryCandidate} onOpenChange={(open) => !open && setViewHistoryCandidate(null)}>
                <DialogContent className="max-w-lg">
                    <DialogHeader>
                        <DialogTitle className="flex items-center gap-2">
                            <History className="h-5 w-5 text-primary" />
                            {t('candidate_profile.history_title')}: {viewHistoryCandidate?.name}
                        </DialogTitle>
                    </DialogHeader>
                    <div className="space-y-3 max-h-[60vh] overflow-y-auto">
                        {isHistoryLoading ? (
                            <div className="flex justify-center py-8">
                                <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
                            </div>
                        ) : history?.length === 0 ? (
                            <p className="text-center text-muted-foreground py-4">{t('candidate_profile.no_history')}</p>
                        ) : (
                            history?.map((item, index) => (
                                <div key={index} className="flex gap-3 border-l-2 border-primary/20 pl-3 py-2 hover:bg-muted/50 rounded-r-lg transition-colors">
                                    <div className="flex-shrink-0 mt-1">
                                        {getHistoryIcon(item.event_type, item.status)}
                                    </div>
                                    <div className="flex-1 min-w-0">
                                        <div className="flex items-center justify-between gap-2">
                                            <span className="font-medium text-sm truncate">
                                                {item.event_type === 'registration' && t('candidate_profile.event_registered')}
                                                {item.event_type === 'application' && t('candidate_profile.event_applied')}
                                                {item.event_type === 'profile_update' && t('candidate_profile.event_update')}
                                                {item.event_type === 'test_attempt' && t('candidate_profile.event_test')}
                                                {!['registration', 'application', 'profile_update', 'test_attempt'].includes(item.event_type) && item.title}
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

                                            // Score localization
                                            if (item.event_type === 'test_attempt') {
                                                if (item.metadata?.percentage !== undefined) {
                                                    return <p className="text-xs text-muted-foreground mt-0.5">{t('dashboard.attempts.labels.score')}: {item.metadata.percentage}%</p>;
                                                }
                                                // Fallback for old string format
                                                if (item.description.startsWith("Score:")) {
                                                    return <p className="text-xs text-muted-foreground mt-0.5">{t('dashboard.attempts.labels.score')}: {item.description.split(': ')[1]}</p>;
                                                }
                                            }

                                            // Vacancy ID localization
                                            if (item.description.startsWith("Vacancy ID:")) {
                                                const id = item.description.split(': ')[1].trim();
                                                return <p className="text-xs text-muted-foreground mt-0.5">{t('common.vacancy')} ID: {id}</p>;
                                            }

                                            // Render raw if no match
                                            return <p className="text-xs text-muted-foreground mt-0.5">{item.description}</p>;
                                        })()}
                                        <p className="text-xs text-muted-foreground/70 mt-1">
                                            {format(new Date(item.timestamp), "PPp", { locale: dateLocale })}
                                        </p>
                                    </div>
                                </div>
                            ))
                        )}
                    </div>
                </DialogContent>
            </Dialog>
        </div>
    )
}

export default function CandidatesPage() {
    return (
        <Suspense fallback={<div className="p-6">Loading...</div>}>
            <CandidatesPageContent />
        </Suspense>
    )
}
