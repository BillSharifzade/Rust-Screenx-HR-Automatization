"use client"
import { cn } from "@/lib/utils"

import { useState, useMemo, useEffect, useRef, Suspense } from "react"
import { motion, AnimatePresence } from "framer-motion"
import { ru as localeRu, enUS as localeEn } from "date-fns/locale"

import { useQuery } from "@tanstack/react-query"
import {
    Search, User, Mail, Phone, Calendar, Briefcase, FileText,
    Download, Sparkles, Binary, Loader2, LayoutGrid,
    List, History, CheckCircle2, Clock, XCircle, Send, MessageSquare,
    AlertCircle, Cake, ChevronDown, Trash2
} from "lucide-react"
import { format } from "date-fns"
import Link from "next/link"
import { useSearchParams } from "next/navigation"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog"
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
} from "@/components/ui/alert-dialog"
import { Textarea } from "@/components/ui/textarea"
import { CandidateApplication, ExternalVacancyListResponse } from "@/types/api"
import { apiFetch, deleteCandidate } from "@/lib/api"
import { useTranslation } from "@/lib/i18n-context"
import { useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select"
import { Skeleton } from "@/components/ui/skeleton"

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
    const processedHighlightRef = useRef<string | null>(null)

    const [searchQuery, setSearchQuery] = useState("")
    const [statusFilter, setStatusFilter] = useState<string>("all")
    const [vacancyFilter, setVacancyFilter] = useState<string>("all")
    const [layout, setLayoutState] = useState<'grid' | 'list'>('grid')
    const [viewVacanciesCandidate, setViewVacanciesCandidate] = useState<any>(null);
    const [viewHistoryCandidate, setViewHistoryCandidate] = useState<any>(null);
    const [messageCandidate, setMessageCandidate] = useState<any>(null);
    const [messageText, setMessageText] = useState("");
    const [isSendingMessage, setIsSendingMessage] = useState(false);
    const [isBlurring, setIsBlurring] = useState(false)
    const [isAnalyzingId, setIsAnalyzingId] = useState<string | null>(null)
    const chatContainerRef = useRef<HTMLDivElement>(null)
    const queryClient = useQueryClient()
    const { language } = useTranslation()

    // Fetch chat messages when message dialog is open
    const { data: chatMessages, isLoading: isChatLoading, refetch: refetchChat } = useQuery({
        queryKey: ['chat-messages', messageCandidate?.id],
        queryFn: () => apiFetch<any[]>(`/api/integration/messages/${messageCandidate?.id}`),
        enabled: !!messageCandidate?.id,
        refetchInterval: 5000, // Poll every 5 seconds for new messages
    });

    // Scroll chat to bottom when messages load
    useEffect(() => {
        if (chatContainerRef.current && chatMessages?.length) {
            chatContainerRef.current.scrollTop = chatContainerRef.current.scrollHeight;
        }
    }, [chatMessages]);

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

    const MessageBadge = ({ count }: { count?: number }) => {
        if (!count || count <= 0) return null;
        return (
            <span className="absolute -top-1.5 -right-1.5 flex h-4 w-4 items-center justify-center rounded-full bg-rose-500 text-[10px] font-bold text-white shadow-sm ring-2 ring-background animate-in zoom-in duration-300 z-10">
                {count > 9 ? '9+' : count}
            </span>
        );
    };

    const getHistoryIcon = (eventType: string, status: string | null) => {
        const s = status || "";
        switch (eventType) {
            case 'registration':
                return <User className="h-4 w-4 text-green-500" />;
            case 'application':
                return <Briefcase className="h-4 w-4 text-blue-500" />;
            case 'profile_update':
                return <FileText className="h-4 w-4 text-purple-500" />;
            case 'test_attempt':
                if (s === 'Passed' || s.includes('status_passed')) return <CheckCircle2 className="h-4 w-4 text-green-500" />;
                if (s === 'Failed' || s.includes('status_failed')) return <XCircle className="h-4 w-4 text-red-500" />;
                if (s === 'Pending' || s.includes('statuses.pending')) return <Clock className="h-4 w-4 text-yellow-500" />;
                return <AlertCircle className="h-4 w-4 text-orange-500" />;
            default:
                return <History className="h-4 w-4 text-muted-foreground" />;
        }
    };

    const getVacancyTitle = (id: number) => {
        return vacancyData?.vacancies.find(v => v.id === id)?.title || `${t('common.vacancy')} #${id}`;
    };

    const handleStatusUpdate = async (candidateId: string, newStatus: string) => {
        try {
            await apiFetch(`/api/integration/candidates/${candidateId}/status`, {
                method: 'POST',
                body: JSON.stringify({ status: newStatus })
            });
            queryClient.invalidateQueries({ queryKey: ["candidates"] });
        } catch (e) {
            console.error("Failed to update status", e);
        }
    };

    const renderStatus = (status: string) => {
        const variants: Record<string, { color: string; glow: string; label: string }> = {
            new: { color: 'bg-blue-500', glow: 'shadow-blue-500/50', label: t('dashboard.candidates.statuses.new') },
            reviewing: { color: 'bg-amber-500', glow: 'shadow-amber-500/50', label: t('dashboard.candidates.statuses.reviewing') },
            contacted: { color: 'bg-emerald-500', glow: 'shadow-emerald-500/50', label: t('dashboard.candidates.statuses.contacted') },
            rejected: { color: 'bg-rose-500', glow: 'shadow-rose-500/50', label: t('dashboard.candidates.statuses.rejected') },
            accepted: { color: 'bg-emerald-600', glow: 'shadow-emerald-600/50', label: t('dashboard.candidates.statuses.accepted') },
        };
        const config = variants[status] || { color: 'bg-muted-foreground', glow: 'shadow-muted-foreground/30', label: status };
        return (
            <div className="flex items-center gap-2.5" title={config.label}>
                <div className={cn("h-2.5 w-2.5 rounded-full shrink-0 shadow-[0_0_10px_-1px] animate-pulse-slow", config.color, config.glow)} />
                <span className="text-xs font-semibold text-foreground tracking-wide">{config.label}</span>
            </div>
        );
    };

    const handleAiAnalyze = async (candidateId: string) => {
        try {
            setIsAnalyzingId(candidateId);
            const candidate = (queryClient.getQueryData(["candidates"]) as any[] || []).find(c => c.id === candidateId);
            if (candidate?.status === 'new') {
                await handleStatusUpdate(candidateId, 'reviewing');
            }
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
            const candidateId = messageCandidate.id;
            if (messageCandidate.status === 'new') {
                await handleStatusUpdate(candidateId, 'reviewing');
            }
            await apiFetch(`/api/integration/messages`, {
                method: 'POST',
                body: JSON.stringify({
                    candidate_id: candidateId,
                    text: messageText
                })
            });
            setMessageText("");
            // Refetch chat messages to show the sent message
            refetchChat();
        } catch (e) {
            console.error(e);
            toast.error(t('common.error') || "Failed to send message");
        } finally {
            setIsSendingMessage(false);
        }
    };

    const handleDeleteCandidate = async (candidateId: string) => {
        try {
            await deleteCandidate(candidateId);
            toast.success(t('candidate_profile.deleted_success') || "Candidate deleted successfully");
            queryClient.invalidateQueries({ queryKey: ["candidates"] });
        } catch (e) {
            console.error(e);
            toast.error(t('candidate_profile.delete_error') || "Failed to delete candidate");
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

    const DeleteCandidateDialog = ({ candidate, trigger }: { candidate: any, trigger: React.ReactNode }) => (
        <AlertDialog>
            <AlertDialogTrigger asChild>
                {trigger}
            </AlertDialogTrigger>
            <AlertDialogContent>
                <AlertDialogHeader>
                    <AlertDialogTitle>{t('candidate_profile.delete_confirm_title') || "Are you absolutely sure?"}</AlertDialogTitle>
                    <AlertDialogDescription>
                        {t('candidate_profile.delete_confirm_desc') || `This will permanently delete ${candidate.name} and all associated data. This action cannot be undone.`}
                    </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                    <AlertDialogCancel>{t('common.cancel') || "Cancel"}</AlertDialogCancel>
                    <AlertDialogAction
                        onClick={() => handleDeleteCandidate(candidate.id)}
                        className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                    >
                        {t('common.delete') || "Delete"}
                    </AlertDialogAction>
                </AlertDialogFooter>
            </AlertDialogContent>
        </AlertDialog>
    );

    const { data: candidates, isLoading } = useQuery({
        queryKey: ["candidates"],
        queryFn: fetchCandidates,
    })

    // Handle highlight and scroll
    useEffect(() => {
        if (highlightId && candidates && processedHighlightRef.current !== highlightId) {
            processedHighlightRef.current = highlightId;
            // Clear any search that might hide the candidate
            setSearchQuery("");

            const timers: NodeJS.Timeout[] = [];

            // Wait for DOM to stabilize and search to clear
            const mainTimer = setTimeout(() => {
                const cardEl = cardRefs.current.get(highlightId)
                if (cardEl) {
                    cardEl.scrollIntoView({ behavior: 'smooth', block: 'center' })

                    const highlightTimer = setTimeout(() => {
                        setHighlightedId(highlightId)
                        setIsBlurring(true)

                        const blurTimer = setTimeout(() => setIsBlurring(false), 2000)
                        const resetTimer = setTimeout(() => {
                            setHighlightedId(null)
                        }, 3000)

                        timers.push(blurTimer, resetTimer);
                    }, 300)

                    timers.push(highlightTimer);
                }
            }, 600)

            timers.push(mainTimer);
            return () => timers.forEach(t => clearTimeout(t));
        }
    }, [highlightId, candidates])

    // Robust search - matches name, email, phone, and telegram ID
    const filteredCandidates = useMemo(() => {
        let result = candidates || [];

        if (statusFilter !== "all") {
            result = result.filter((c: any) => c.status === statusFilter);
        }

        if (vacancyFilter !== "all") {
            const vid = parseInt(vacancyFilter);
            result = result.filter((c: any) => c.vacancy_id === vid);
        }

        if (!searchQuery.trim()) return result;

        const query = searchQuery.toLowerCase().trim()

        return result.filter((candidate: any) => {
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
    }, [candidates, searchQuery, statusFilter])

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
                    <div className="flex items-center gap-2">
                        <div className="relative w-64">
                            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
                            <Input
                                placeholder={t('dashboard.candidates.search_placeholder')}
                                className="pl-8 bg-muted/30 focus:bg-background transition-colors"
                                value={searchQuery}
                                onChange={(e) => setSearchQuery(e.target.value)}
                            />
                        </div>
                        <Select value={statusFilter} onValueChange={setStatusFilter}>
                            <SelectTrigger className="w-[160px] bg-muted/30 transition-colors">
                                <SelectValue placeholder={t('common.filter') || "Filter"} />
                            </SelectTrigger>
                            <SelectContent>
                                <SelectItem value="all">{t('common.all_statuses') || "All Statuses"}</SelectItem>
                                {['new', 'reviewing', 'contacted', 'rejected', 'accepted'].map((status) => (
                                    <SelectItem key={status} value={status}>
                                        {t(`dashboard.candidates.statuses.${status}`)}
                                    </SelectItem>
                                ))}
                            </SelectContent>
                        </Select>

                        <Select value={vacancyFilter} onValueChange={setVacancyFilter}>
                            <SelectTrigger className="w-[220px] bg-muted/30 transition-colors">
                                <SelectValue placeholder={t('common.vacancy') || "Vacancy"} />
                            </SelectTrigger>
                            <SelectContent className="max-w-[400px]">
                                <SelectItem value="all">{t('common.all_vacancies')}</SelectItem>
                                {vacancyData?.vacancies.map((v) => (
                                    <SelectItem key={v.id} value={v.id.toString()}>
                                        <span className="text-secondary-foreground font-medium truncate block">
                                            {v.title.replace(/<\/?[^>]+(>|$)/g, "")}
                                        </span>
                                    </SelectItem>
                                ))}
                            </SelectContent>
                        </Select>
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
                                opacity: isBlurring && highlightedId !== candidate.id ? 0.4 : 1,
                                scale: highlightedId === candidate.id ? [1, 1.05, 1.02] : 1,
                                filter: isBlurring && highlightedId !== candidate.id ? 'blur(8px)' : 'blur(0px)',
                                backgroundColor: highlightedId === candidate.id ? "hsl(var(--primary) / 0.05)" : "transparent",
                                borderColor: highlightedId === candidate.id ? "hsl(var(--primary))" : "transparent",
                                boxShadow: highlightedId === candidate.id ? "0 0 30px hsl(var(--primary) / 0.2)" : "none",
                                y: 0
                            }}
                            exit={{ opacity: 0, scale: 0.95 }}
                            transition={{
                                duration: 0.3,
                                ease: "easeOut"
                            }}
                            ref={(el) => { if (el) cardRefs.current.set(candidate.id, el) }}
                            className={cn(
                                "rounded-xl overflow-hidden premium-hover border-2 relative transition-all duration-300",
                                highlightedId === candidate.id
                                    ? "z-50 ring-4 ring-primary/20 scale-[1.02]"
                                    : "border-transparent z-0",
                                layout === 'list' && "w-full"
                            )}
                        >
                            <Card className={cn(
                                "border-0 shadow-none rounded-none bg-card/40 backdrop-blur-md h-full transition-all duration-300 group",
                                layout === 'list' ? "p-2" : "p-0"
                            )}>
                                {layout === 'grid' ? (
                                    <div className="flex flex-col h-full">
                                        <CardHeader className="pb-3 bg-muted/30 relative h-[72px] flex flex-row items-center border-b border-primary/5">
                                            <div className="flex items-center gap-3 pr-28 min-w-0 flex-1">
                                                <div className="h-10 w-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0 shadow-inner">
                                                    <User className="h-6 w-6 text-primary" />
                                                </div>
                                                <div className="flex-1 min-w-0">
                                                    <CardTitle className="text-base font-bold leading-tight truncate">{candidate.name}</CardTitle>
                                                </div>
                                            </div>

                                            <div className="absolute top-1/2 -translate-y-1/2 right-3 flex items-center gap-0.5 bg-background/50 backdrop-blur-md p-0.5 rounded-lg border border-primary/10 shadow-sm transition-all hover:bg-background/80">
                                                <DropdownMenu>
                                                    <DropdownMenuTrigger asChild>
                                                        <Button variant="ghost" size="sm" className="h-8 w-8 p-0 rounded-md hover:bg-primary/10">
                                                            <ChevronDown className="h-4 w-4 text-muted-foreground" />
                                                        </Button>
                                                    </DropdownMenuTrigger>
                                                    <DropdownMenuContent align="end" className="w-40">
                                                        {['new', 'reviewing', 'contacted', 'rejected', 'accepted'].map((status) => (
                                                            <DropdownMenuItem
                                                                key={status}
                                                                onClick={() => handleStatusUpdate(candidate.id, status)}
                                                                className={cn("text-xs", candidate.status === status && "bg-muted font-bold")}
                                                            >
                                                                {t(`dashboard.candidates.statuses.${status}`)}
                                                            </DropdownMenuItem>
                                                        ))}
                                                    </DropdownMenuContent>
                                                </DropdownMenu>

                                                <div className="relative">
                                                    <Button variant="ghost" size="sm" className="h-8 w-8 p-0 rounded-md hover:bg-primary/10" onClick={() => {
                                                        if (candidate.status === 'new') handleStatusUpdate(candidate.id, 'reviewing');
                                                        setMessageCandidate(candidate);
                                                    }}>
                                                        <MessageSquare className="h-4 w-4 text-primary" />
                                                    </Button>
                                                    <MessageBadge count={candidate.unread_messages} />
                                                </div>

                                                <AiAssessmentDialog
                                                    candidate={candidate}
                                                    trigger={
                                                        <Button variant="ghost" size="sm" className="h-8 w-8 p-0 rounded-md hover:bg-primary/10 group/ai" onClick={() => {
                                                            if (candidate.status === 'new') handleStatusUpdate(candidate.id, 'reviewing');
                                                        }}>
                                                            <Sparkles className={cn("h-4 w-4 text-primary transition-colors", isAnalyzingId === candidate.id && "animate-pulse")} />
                                                        </Button>
                                                    }
                                                />
                                                <DeleteCandidateDialog
                                                    candidate={candidate}
                                                    trigger={
                                                        <Button variant="ghost" size="sm" className="h-8 w-8 p-0 rounded-md hover:bg-destructive/10 group/delete">
                                                            <Trash2 className="h-4 w-4 text-muted-foreground group-hover/delete:text-destructive transition-colors" />
                                                        </Button>
                                                    }
                                                />
                                            </div>
                                        </CardHeader>
                                        <CardContent className="pt-4 space-y-4 flex-1">
                                            <div className="grid gap-2 text-sm">
                                                <div className="flex items-center gap-2 text-muted-foreground">
                                                    <AlertCircle className="h-4 w-4 shrink-0" />
                                                    {renderStatus(candidate.status)}
                                                </div>
                                                <div className="flex items-center gap-2 text-muted-foreground">
                                                    <Mail className="h-4 w-4 shrink-0" />
                                                    <span className="text-foreground truncate">{candidate.email}</span>
                                                </div>
                                                <div className="flex items-center gap-2 text-muted-foreground">
                                                    <Phone className="h-4 w-4 shrink-0" />
                                                    <span className="text-foreground">{candidate.phone || "—"}</span>
                                                </div>
                                                <div className="flex items-center gap-2 text-muted-foreground">
                                                    <Cake className="h-4 w-4 shrink-0" />
                                                    <span className="text-foreground">
                                                        {t('dashboard.candidates.dob')}: {candidate.dob ? format(new Date(candidate.dob), "dd.MM.yyyy") : "—"}
                                                    </span>
                                                </div>
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
                                                    onClick={() => {
                                                        if (candidate.status === 'new') handleStatusUpdate(candidate.id, 'reviewing');
                                                        setViewVacanciesCandidate(candidate);
                                                    }}
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
                                    <div className="flex flex-col lg:flex-row items-center gap-4 p-4 w-full">
                                        {/* Name Section - Fixed Width */}
                                        <div className="flex items-center gap-4 w-full lg:w-[240px] shrink-0 relative group/header">
                                            <div className="h-11 w-11 rounded-full bg-primary/10 flex items-center justify-center shrink-0 shadow-inner border border-primary/5">
                                                <User className="h-6 w-6 text-primary" />
                                            </div>
                                            <div className="min-w-0 flex-1">
                                                <CardTitle className="text-base font-bold truncate group-hover/header:text-primary transition-colors cursor-default" title={candidate.name}>
                                                    {candidate.name}
                                                </CardTitle>
                                                <div className="mt-1 flex items-center gap-1.5">
                                                    {candidate.status === 'new' && (
                                                        <Badge variant="outline" className="text-[9px] h-4 px-1.5 bg-blue-500/10 text-blue-600 border-blue-500/20 uppercase tracking-tighter">New</Badge>
                                                    )}
                                                </div>
                                            </div>

                                            {/* Quick Actions for Name Section */}
                                            <div className="absolute top-0 right-0 flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity bg-background/80 backdrop-blur-sm p-0.5 rounded-md border border-primary/10">
                                                <DropdownMenu>
                                                    <DropdownMenuTrigger asChild>
                                                        <Button variant="ghost" size="sm" className="h-6 w-6 p-0">
                                                            <ChevronDown className="h-3 w-3 text-muted-foreground" />
                                                        </Button>
                                                    </DropdownMenuTrigger>
                                                    <DropdownMenuContent align="end" className="w-40">
                                                        {['new', 'reviewing', 'contacted', 'rejected', 'accepted'].map((status) => (
                                                            <DropdownMenuItem
                                                                key={status}
                                                                onClick={() => handleStatusUpdate(candidate.id, status)}
                                                                className={cn("text-xs", candidate.status === status && "bg-muted font-bold")}
                                                            >
                                                                {t(`dashboard.candidates.statuses.${status}`)}
                                                            </DropdownMenuItem>
                                                        ))}
                                                    </DropdownMenuContent>
                                                </DropdownMenu>
                                            </div>
                                        </div>

                                        {/* info Grid - Standardized Columns */}
                                        <div className="grid grid-cols-2 md:grid-cols-3 xl:grid-cols-5 gap-4 flex-1 w-full lg:border-l px-0 lg:px-6 border-muted-foreground/10 py-1">
                                            <div className="flex flex-col space-y-1">
                                                <span className="text-[10px] text-muted-foreground/50 uppercase tracking-widest font-bold">Status</span>
                                                <div className="h-6 flex items-center">
                                                    {renderStatus(candidate.status)}
                                                </div>
                                            </div>
                                            <div className="flex flex-col space-y-1 min-w-0">
                                                <span className="text-[10px] text-muted-foreground/50 uppercase tracking-widest font-bold">Email</span>
                                                <div className="flex items-center gap-2 h-6 min-w-0">
                                                    <Mail className="h-3.5 w-3.5 text-primary/60 shrink-0" />
                                                    <span className="text-xs text-foreground truncate" title={candidate.email}>{candidate.email}</span>
                                                </div>
                                            </div>
                                            <div className="flex flex-col space-y-1">
                                                <span className="text-[10px] text-muted-foreground/50 uppercase tracking-widest font-bold">{t('registration.phone') || 'Phone'}</span>
                                                <div className="flex items-center gap-2 h-6">
                                                    <Phone className="h-3.5 w-3.5 text-primary/60 shrink-0" />
                                                    <span className="text-xs text-foreground whitespace-nowrap">{candidate.phone || "—"}</span>
                                                </div>
                                            </div>
                                            <div className="flex flex-col space-y-1">
                                                <span className="text-[10px] text-muted-foreground/50 uppercase tracking-widest font-bold whitespace-nowrap">{t('dashboard.candidates.registered_at')}</span>
                                                <div className="flex items-center gap-2 h-6">
                                                    <Calendar className="h-3.5 w-3.5 text-primary/60 shrink-0" />
                                                    <span className="text-xs text-foreground whitespace-nowrap">{format(new Date(candidate.created_at), "dd.MM.yyyy")}</span>
                                                </div>
                                            </div>
                                            <div className="flex flex-col space-y-1">
                                                <span className="text-[10px] text-muted-foreground/50 uppercase tracking-widest font-bold whitespace-nowrap">{t('dashboard.candidates.dob')}</span>
                                                <div className="flex items-center gap-2 h-6">
                                                    <Cake className="h-3.5 w-3.5 text-primary/60 shrink-0" />
                                                    <span className="text-xs text-foreground whitespace-nowrap">
                                                        {candidate.dob ? format(new Date(candidate.dob), "dd.MM.yyyy") : "—"}
                                                    </span>
                                                </div>
                                            </div>
                                        </div>

                                        {/* Actions Section - Standardized spacing */}
                                        <div className="flex items-center gap-3 w-full lg:w-auto shrink-0 lg:border-l pl-0 lg:pl-6 border-muted-foreground/10">
                                            <div className="flex items-center gap-2 w-full lg:w-auto">
                                                {/* Tool Buttons */}
                                                <div className="flex items-center gap-1.5 bg-muted/30 p-1 rounded-lg border border-primary/5">
                                                    {candidate.cv_url ? (
                                                        <Button variant="ghost" size="sm" className="h-8 w-8 p-0 hover:bg-background shadow-sm" asChild>
                                                            <a href={`/${candidate.cv_url?.replace(/^\.?\/?/, '')}`} target="_blank" rel="noopener noreferrer" title={t('dashboard.candidates.cv_download')}>
                                                                <Download className="h-3.5 w-3.5 text-primary" />
                                                            </a>
                                                        </Button>
                                                    ) : (
                                                        <Button variant="ghost" size="sm" className="h-8 w-8 p-0 opacity-40" disabled>
                                                            <FileText className="h-3.5 w-3.5" />
                                                        </Button>
                                                    )}
                                                    <Button variant="ghost" size="sm" className="h-8 w-8 p-0 hover:bg-background shadow-sm" onClick={() => setViewHistoryCandidate(candidate)} title={t('candidate_profile.history')}>
                                                        <History className="h-3.5 w-3.5 text-primary" />
                                                    </Button>
                                                    <AiAssessmentDialog
                                                        candidate={candidate}
                                                        trigger={
                                                            <Button variant="ghost" size="sm" className="h-8 w-8 p-0 hover:bg-background shadow-sm group/ai" onClick={() => {
                                                                if (candidate.status === 'new') handleStatusUpdate(candidate.id, 'reviewing');
                                                            }}>
                                                                <Sparkles className={cn("h-3.5 w-3.5 text-primary transition-colors", isAnalyzingId === candidate.id && "animate-pulse")} />
                                                            </Button>
                                                        }
                                                    />
                                                    <DeleteCandidateDialog
                                                        candidate={candidate}
                                                        trigger={
                                                            <Button variant="ghost" size="sm" className="h-8 w-8 p-0 hover:bg-destructive/10 group/delete shadow-sm" title={t('common.delete')}>
                                                                <Trash2 className="h-3.5 w-3.5 text-muted-foreground group-hover/delete:text-destructive transition-colors" />
                                                            </Button>
                                                        }
                                                    />
                                                </div>

                                                {/* Text Buttons */}
                                                <div className="flex items-center gap-2 ml-1">
                                                    <Button variant="outline" size="sm" className="h-9 gap-2 text-[11px] font-semibold border-primary/10 hover:bg-primary/5 hover:border-primary/20 hidden xl:flex relative" onClick={() => {
                                                        if (candidate.status === 'new') handleStatusUpdate(candidate.id, 'reviewing');
                                                        setMessageCandidate(candidate);
                                                    }}>
                                                        <MessageSquare className="h-3.5 w-3.5 text-primary" />
                                                        <span>{t('common.message') || "Message"}</span>
                                                        <MessageBadge count={candidate.unread_messages} />
                                                    </Button>

                                                    <Button variant="secondary" size="sm" className="h-9 gap-2 text-[11px] font-semibold flex-1 lg:flex-none lg:min-w-[130px] border border-transparent hover:border-primary/20" onClick={() => {
                                                        if (candidate.status === 'new') handleStatusUpdate(candidate.id, 'reviewing');
                                                        setViewVacanciesCandidate(candidate);
                                                    }}>
                                                        <Briefcase className="h-3.5 w-3.5 text-primary" />
                                                        <span>{t('dashboard.candidates.applied_vacancies')}</span>
                                                    </Button>

                                                    <Button variant="default" size="sm" className="h-9 gap-2 text-[11px] font-bold shadow-md shadow-primary/10 flex-1 lg:flex-none lg:min-w-[110px]" asChild onClick={() => {
                                                        if (candidate.status === 'new') handleStatusUpdate(candidate.id, 'reviewing');
                                                    }}>
                                                        <Link href={`/dashboard/invites?candidate=${candidate.id}`}>
                                                            <Send className="h-3.5 w-3.5" />
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
                <DialogContent className="sm:max-w-lg max-h-[85vh] flex flex-col">
                    <DialogHeader>
                        <DialogTitle className="flex items-center gap-2">
                            <MessageSquare className="h-5 w-5 text-primary" />
                            {t('dashboard.candidates.chat_title') || "Chat"} - {messageCandidate?.name}
                        </DialogTitle>
                        <DialogDescription>
                            {t('dashboard.candidates.chat_desc') || "Messages are delivered via Telegram bot."}
                        </DialogDescription>
                    </DialogHeader>

                    <div
                        ref={chatContainerRef}
                        className="flex-1 flex flex-col overflow-y-auto min-h-[300px] max-h-[400px] space-y-3 py-4 px-2 bg-muted/30 rounded-lg mt-4"
                    >
                        {isChatLoading ? (
                            <div className="space-y-4 px-2">
                                <div className="flex flex-col gap-2 items-start max-w-[80%] mr-auto">
                                    <Skeleton className="h-10 w-48 rounded-lg" />
                                    <Skeleton className="h-3 w-12" />
                                </div>
                                <div className="flex flex-col gap-2 items-end max-w-[80%] ml-auto">
                                    <Skeleton className="h-14 w-64 rounded-lg" />
                                    <Skeleton className="h-3 w-12" />
                                </div>
                                <div className="flex flex-col gap-2 items-start max-w-[80%] mr-auto">
                                    <Skeleton className="h-8 w-32 rounded-lg" />
                                    <Skeleton className="h-3 w-12" />
                                </div>
                            </div>
                        ) : chatMessages?.length === 0 ? (
                            <div className="flex flex-col items-center justify-center flex-1 h-full text-muted-foreground my-auto">
                                <MessageSquare className="h-8 w-8 mb-2 opacity-50" />
                                <p className="text-sm">{t('dashboard.candidates.no_messages') || "No messages yet"}</p>
                            </div>
                        ) : (
                            chatMessages?.map((msg: any) => (
                                <div
                                    key={msg.id}
                                    className={cn(
                                        "flex flex-col max-w-[80%] rounded-lg px-3 py-2",
                                        msg.direction === 'outbound'
                                            ? "ml-auto bg-primary text-primary-foreground"
                                            : "mr-auto bg-background border"
                                    )}
                                >
                                    <p className="text-sm whitespace-pre-wrap break-words">{msg.text}</p>
                                    <span className={cn(
                                        "text-[10px] mt-1",
                                        msg.direction === 'outbound' ? "text-primary-foreground/70" : "text-muted-foreground"
                                    )}>
                                        {format(new Date(msg.created_at), "HH:mm, d MMM", { locale: dateLocale })}
                                    </span>
                                </div>
                            ))
                        )}
                    </div>

                    {/* Input area */}
                    <div className="flex gap-2 pt-2 border-t">
                        <Textarea
                            placeholder={t('dashboard.candidates.message_placeholder') || "Type a message..."}
                            value={messageText}
                            onChange={(e) => setMessageText(e.target.value)}
                            onKeyDown={(e) => {
                                if (e.key === 'Enter' && !e.shiftKey) {
                                    e.preventDefault();
                                    handleSendMessage();
                                }
                            }}
                            className="min-h-[60px] max-h-[100px] resize-none flex-1"
                        />
                        <Button
                            onClick={handleSendMessage}
                            disabled={!messageText.trim() || isSendingMessage}
                            size="icon"
                            className="h-[60px] w-[60px] shrink-0"
                        >
                            {isSendingMessage ? <Loader2 className="h-5 w-5 animate-spin" /> : <Send className="h-5 w-5" />}
                        </Button>
                    </div>
                </DialogContent>
            </Dialog>

            <Dialog open={!!viewVacanciesCandidate} onOpenChange={(open) => !open && setViewVacanciesCandidate(null)}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>{t('dashboard.candidates.applied_vacancies')}: {viewVacanciesCandidate?.name}</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4 max-h-[60vh] overflow-y-auto">
                        {applicationsLoading ? (
                            <div className="text-center py-4">{t('common.loading')}</div>
                        ) : applications?.length === 0 ? (
                            <p className="text-center text-muted-foreground py-4">{t('candidate_profile.no_applications')}</p>
                        ) : (
                            applications?.map((app) => (
                                <div key={app.id} className="border rounded-lg p-3 space-y-1">
                                    <div className="font-medium" dangerouslySetInnerHTML={{ __html: getVacancyTitle(app.vacancy_id) }} />
                                    <div className="text-xs text-muted-foreground">
                                        {t('candidate_profile.applied_on')} {format(new Date(app.created_at), "PPP", { locale: dateLocale })}
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
                                                {t(item.title)}
                                            </span>
                                            {item.status && (
                                                <Badge variant="outline" className="text-xs flex-shrink-0">
                                                    {(() => {
                                                        const s = item.status;
                                                        if (!s) return null;
                                                        if (s.includes('.')) return t(s);

                                                        // Legacy fallbacks
                                                        if (s === 'Passed') return t('dashboard.attempts.labels.passed');
                                                        if (s === 'Failed') return t('dashboard.attempts.labels.failed');
                                                        return t(s) || s;
                                                    })()}
                                                </Badge>
                                            )}
                                        </div>
                                        {(() => {
                                            if (!item.description) {
                                                if (item.event_type === 'profile_update') {
                                                    return <p className="text-xs text-muted-foreground mt-0.5">{t('candidate_profile.event_update_desc')}</p>;
                                                }
                                                return null;
                                            }

                                            // Registration email localization
                                            if (item.event_type === 'registration') {
                                                return <p className="text-xs text-muted-foreground mt-0.5">
                                                    {t('candidate_profile.event_registered_desc').replace('{email}', item.description)}
                                                </p>;
                                            }

                                            // Application vacancy ID localization
                                            if (item.event_type === 'application') {
                                                return <p className="text-xs text-muted-foreground mt-0.5">
                                                    {t('candidate_profile.event_applied_desc').replace('{id}', item.description)}
                                                </p>;
                                            }

                                            // Score localization
                                            if (item.event_type === 'test_attempt') {
                                                return <p className="text-xs text-muted-foreground mt-0.5">{t('dashboard.attempts.labels.score')}: {item.description}%</p>;
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
