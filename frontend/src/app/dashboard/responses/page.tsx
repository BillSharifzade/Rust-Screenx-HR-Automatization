"use client"

import { useMemo, useState } from "react"
import { motion, AnimatePresence } from "framer-motion"
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { format } from "date-fns"
import type { Locale } from "date-fns"
import { ru as localeRu, enUS as localeEn } from "date-fns/locale"
import {
    ChevronLeft, ChevronRight, Sparkles, Loader2, FileText,
    Check, X, MoreHorizontal, Clock, User as UserIcon, Briefcase, MessageSquare,
} from "lucide-react"

import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Textarea } from "@/components/ui/textarea"
import {
    Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogTrigger,
} from "@/components/ui/dialog"
import {
    DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { listResponses, updateResponse, ResponseCard, ResponsesFeed, UpdateResponseBody } from "@/lib/api"
import { useTranslation } from "@/lib/i18n-context"
import { toast } from "sonner"
import { cn } from "@/lib/utils"

const QK = ["responses"] as const

const DEFAULT_STAGES = [
    "cv_screening", "phone_interview", "interview_1", "test_task",
    "presentation", "interview_2", "final_decision",
]

function gradeClasses(grade?: number | null): string {
    if (grade == null) return "bg-muted text-muted-foreground"
    if (grade >= 80) return "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 border-emerald-500/30"
    if (grade >= 60) return "bg-amber-500/15 text-amber-600 dark:text-amber-400 border-amber-500/30"
    if (grade >= 30) return "bg-orange-500/15 text-orange-600 dark:text-orange-400 border-orange-500/30"
    return "bg-rose-500/15 text-rose-600 dark:text-rose-400 border-rose-500/30"
}

export default function ResponsesPage() {
    const { t, language } = useTranslation()
    const qc = useQueryClient()
    const locale = language === "ru" ? localeRu : localeEn

    const { data, isLoading } = useQuery({
        queryKey: QK,
        queryFn: listResponses,
        refetchInterval: 15000,
    })

    const stages = data?.stages ?? DEFAULT_STAGES
    const items = data?.items ?? []

    const mutation = useMutation({
        mutationFn: ({ id, body }: { id: string; body: UpdateResponseBody }) => updateResponse(id, body),
        onMutate: async ({ id, body }) => {
            await qc.cancelQueries({ queryKey: QK })
            const prev = qc.getQueryData<ResponsesFeed>(QK)
            if (prev) {
                qc.setQueryData<ResponsesFeed>(QK, {
                    ...prev,
                    items: prev.items.map((it) =>
                        it.id === id ? { ...it, ...body } as ResponseCard : it
                    ),
                })
            }
            return { prev }
        },
        onError: (_e, _v, ctx) => {
            if (ctx?.prev) qc.setQueryData(QK, ctx.prev)
            toast.error(t("dashboard.responses.update_failed"))
        },
        onSuccess: () => toast.success(t("dashboard.responses.updated")),
        onSettled: () => qc.invalidateQueries({ queryKey: QK }),
    })

    const byStage = useMemo(() => {
        const map: Record<string, ResponseCard[]> = {}
        for (const s of stages) map[s] = []
        for (const it of items) (map[it.status] ??= []).push(it)
        return map
    }, [items, stages])

    const stageLabel = (s: string) => t(`dashboard.responses.stages.${s}`)

    const move = (card: ResponseCard, dir: 1 | -1) => {
        const idx = stages.indexOf(card.status)
        const next = stages[idx + dir]
        if (!next) return
        mutation.mutate({ id: card.id, body: { status: next } })
    }
    const moveTo = (card: ResponseCard, status: string) =>
        mutation.mutate({ id: card.id, body: { status } })
    const decide = (card: ResponseCard, decision: "accepted" | "rejected") =>
        mutation.mutate({ id: card.id, body: { decision } })
    const saveComment = (card: ResponseCard, hr_comment: string) =>
        mutation.mutate({ id: card.id, body: { hr_comment } })

    return (
        <div className="flex flex-col h-[calc(100vh-1rem)] p-4 md:p-6 gap-4">
            <header className="shrink-0">
                <h1 className="text-2xl font-bold tracking-tight flex items-center gap-2">
                    <Sparkles className="h-6 w-6 text-primary" />
                    {t("dashboard.responses.title")}
                </h1>
                <p className="text-sm text-muted-foreground mt-1">{t("dashboard.responses.subtitle")}</p>
            </header>

            {isLoading ? (
                <div className="flex-1 flex items-center justify-center text-muted-foreground">
                    <Loader2 className="h-6 w-6 animate-spin mr-2" /> …
                </div>
            ) : items.length === 0 ? (
                <div className="flex-1 flex items-center justify-center text-muted-foreground">
                    {t("dashboard.responses.empty")}
                </div>
            ) : (
                <div className="flex-1 min-h-0 flex gap-4 overflow-x-auto pb-2">
                    {stages.map((stage, si) => (
                        <div key={stage} className="flex flex-col w-[300px] shrink-0 rounded-xl bg-muted/40 border">
                            <div className="flex items-center justify-between px-3 py-2.5 border-b sticky top-0">
                                <span className="text-sm font-semibold">{stageLabel(stage)}</span>
                                <Badge variant="secondary" className="rounded-full">{byStage[stage]?.length ?? 0}</Badge>
                            </div>
                            <div className="flex-1 overflow-y-auto p-2 space-y-2">
                                <AnimatePresence mode="popLayout">
                                    {(byStage[stage] ?? []).map((card) => (
                                        <ResponseCardView
                                            key={card.id}
                                            card={card}
                                            stages={stages}
                                            stageIndex={si}
                                            t={t}
                                            stageLabel={stageLabel}
                                            dateLocale={locale}
                                            onMove={move}
                                            onMoveTo={moveTo}
                                            onDecide={decide}
                                            onSaveComment={saveComment}
                                        />
                                    ))}
                                </AnimatePresence>
                                {(byStage[stage]?.length ?? 0) === 0 && (
                                    <div className="text-xs text-muted-foreground/60 text-center py-6 select-none">
                                        {t("dashboard.responses.empty_col")}
                                    </div>
                                )}
                            </div>
                        </div>
                    ))}
                </div>
            )}
        </div>
    )
}

function ResponseCardView({
    card, stages, stageIndex, t, stageLabel, dateLocale, onMove, onMoveTo, onDecide, onSaveComment,
}: {
    card: ResponseCard
    stages: string[]
    stageIndex: number
    t: (k: string) => string
    stageLabel: (s: string) => string
    dateLocale: Locale
    onMove: (c: ResponseCard, dir: 1 | -1) => void
    onMoveTo: (c: ResponseCard, status: string) => void
    onDecide: (c: ResponseCard, d: "accepted" | "rejected") => void
    onSaveComment: (c: ResponseCard, comment: string) => void
}) {
    const [commentOpen, setCommentOpen] = useState(false)
    const [draft, setDraft] = useState(card.hr_comment ?? "")
    const isFirst = stageIndex === 0
    const isLast = stageIndex === stages.length - 1
    const isFinal = card.status === "final_decision"

    return (
        <motion.div
            layout
            initial={{ opacity: 0, y: 8, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, scale: 0.96 }}
            transition={{ type: "spring", stiffness: 420, damping: 32 }}
            className="rounded-lg border bg-card shadow-sm hover:shadow-md transition-shadow p-3 space-y-2.5"
        >
            {/* candidate + grade */}
            <div className="flex items-start justify-between gap-2">
                <div className="min-w-0">
                    <div className="font-medium text-sm flex items-center gap-1.5 truncate">
                        <UserIcon className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                        <span className="truncate">{card.candidate_name}</span>
                    </div>
                    <div className="text-xs text-muted-foreground flex items-center gap-1.5 truncate mt-0.5">
                        <Briefcase className="h-3 w-3 shrink-0" />
                        <span className="truncate">{card.vacancy_title || `#${card.vacancy_id}`}</span>
                    </div>
                </div>
                {card.ai_grade != null ? (
                    <Badge variant="outline" className={cn("shrink-0 font-semibold tabular-nums", gradeClasses(card.ai_grade))}>
                        {card.ai_grade}%
                    </Badge>
                ) : (
                    <Badge variant="outline" className="shrink-0 gap-1 text-muted-foreground">
                        <Loader2 className="h-3 w-3 animate-spin" />
                        {t("dashboard.responses.ai_pending")}
                    </Badge>
                )}
            </div>

            {/* AI comment */}
            {card.ai_comment && (
                <p className="text-xs text-muted-foreground leading-snug line-clamp-3 bg-muted/40 rounded-md p-2">
                    <Sparkles className="h-3 w-3 inline mr-1 -mt-0.5 text-primary" />
                    {card.ai_comment}
                </p>
            )}

            {/* HR comment */}
            {card.hr_comment && (
                <p className="text-xs leading-snug line-clamp-2 border-l-2 border-primary/40 pl-2">
                    <MessageSquare className="h-3 w-3 inline mr-1 -mt-0.5" />
                    {card.hr_comment}
                </p>
            )}

            {/* decision badge */}
            {isFinal && card.decision && (
                <Badge
                    variant="outline"
                    className={cn(
                        "w-full justify-center",
                        card.decision === "accepted"
                            ? "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 border-emerald-500/30"
                            : "bg-rose-500/15 text-rose-600 dark:text-rose-400 border-rose-500/30",
                    )}
                >
                    {card.decision === "accepted" ? t("dashboard.responses.accepted") : t("dashboard.responses.rejected")}
                </Badge>
            )}

            {/* meta */}
            <div className="flex items-center gap-1 text-[11px] text-muted-foreground">
                <Clock className="h-3 w-3" />
                {format(new Date(card.responded_at), "d MMM yyyy", { locale: dateLocale })}
                {card.candidate_cv_url && (
                    <a
                        href={card.candidate_cv_url}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="ml-auto inline-flex items-center gap-1 hover:text-primary transition-colors"
                    >
                        <FileText className="h-3 w-3" /> CV
                    </a>
                )}
            </div>

            {/* actions */}
            <div className="flex items-center gap-1 pt-1 border-t">
                <Button
                    variant="ghost" size="icon" className="h-7 w-7"
                    disabled={isFirst}
                    title={t("dashboard.responses.move_prev")}
                    onClick={() => onMove(card, -1)}
                >
                    <ChevronLeft className="h-4 w-4" />
                </Button>

                {isFinal ? (
                    <div className="flex-1 flex gap-1">
                        <Button
                            variant="outline" size="sm"
                            className="h-7 flex-1 text-emerald-600 hover:text-emerald-600 hover:bg-emerald-500/10"
                            onClick={() => onDecide(card, "accepted")}
                        >
                            <Check className="h-3.5 w-3.5 mr-1" /> {t("dashboard.responses.accept")}
                        </Button>
                        <Button
                            variant="outline" size="sm"
                            className="h-7 flex-1 text-rose-600 hover:text-rose-600 hover:bg-rose-500/10"
                            onClick={() => onDecide(card, "rejected")}
                        >
                            <X className="h-3.5 w-3.5 mr-1" /> {t("dashboard.responses.reject")}
                        </Button>
                    </div>
                ) : (
                    <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                            <Button variant="ghost" size="sm" className="h-7 flex-1 text-xs">
                                <MoreHorizontal className="h-3.5 w-3.5 mr-1" />
                                {t("dashboard.responses.move_to")}
                            </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="center">
                            {stages.map((s) => (
                                <DropdownMenuItem
                                    key={s}
                                    disabled={s === card.status}
                                    onClick={() => onMoveTo(card, s)}
                                >
                                    {stageLabel(s)}
                                </DropdownMenuItem>
                            ))}
                        </DropdownMenuContent>
                    </DropdownMenu>
                )}

                <Dialog open={commentOpen} onOpenChange={(o) => { setCommentOpen(o); if (o) setDraft(card.hr_comment ?? "") }}>
                    <DialogTrigger asChild>
                        <Button variant="ghost" size="icon" className="h-7 w-7" title={t("dashboard.responses.edit_comment")}>
                            <MessageSquare className="h-4 w-4" />
                        </Button>
                    </DialogTrigger>
                    <DialogContent>
                        <DialogHeader>
                            <DialogTitle>{t("dashboard.responses.edit_comment")}</DialogTitle>
                        </DialogHeader>
                        <Textarea
                            value={draft}
                            onChange={(e) => setDraft(e.target.value)}
                            placeholder={t("dashboard.responses.comment_placeholder")}
                            rows={4}
                        />
                        <DialogFooter>
                            <Button variant="outline" onClick={() => setCommentOpen(false)}>
                                {t("dashboard.responses.cancel")}
                            </Button>
                            <Button onClick={() => { onSaveComment(card, draft); setCommentOpen(false) }}>
                                {t("dashboard.responses.save")}
                            </Button>
                        </DialogFooter>
                    </DialogContent>
                </Dialog>

                <Button
                    variant="ghost" size="icon" className="h-7 w-7"
                    disabled={isLast}
                    title={t("dashboard.responses.move_next")}
                    onClick={() => onMove(card, 1)}
                >
                    <ChevronRight className="h-4 w-4" />
                </Button>
            </div>
        </motion.div>
    )
}
