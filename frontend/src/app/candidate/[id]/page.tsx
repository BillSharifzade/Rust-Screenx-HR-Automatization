"use client"

import { useEffect, useState } from "react"
import { useParams } from "next/navigation"
import { CandidateProfileCard } from "@/components/CandidateProfileCard"
import { Skeleton } from "@/components/ui/skeleton"
import { AlertCircle, User, Briefcase } from "lucide-react"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { toast } from "sonner"

import { ModeToggle } from "@/components/mode-toggle"
import { LanguageToggle } from "@/components/language-toggle"
import { VacanciesTab } from "@/components/VacanciesTab"
import { ExternalVacancy } from "@/types/api"
import { useTranslation } from "@/lib/i18n-context"

export default function CandidateProfilePage() {
    const { t } = useTranslation()
    const params = useParams()
    const [candidate, setCandidate] = useState<any>(null)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState<string | null>(null)
    const [activeTab, setActiveTab] = useState<'profile' | 'vacancies'>('profile')

    useEffect(() => {
        const fetchCandidate = async () => {
            try {
                // Determine API URL based on environment or proxy
                const apiUrl = process.env.NEXT_PUBLIC_API_URL || ""
                const response = await fetch(`${apiUrl}/api/candidate/${params.id}`)

                if (!response.ok) {
                    throw new Error("Failed to fetch candidate profile")
                }

                const data = await response.json()
                setCandidate(data)
            } catch (err) {
                console.error(err)
                setError("Could not load candidate profile. Please try again later.")
            } finally {
                setLoading(false)
            }
        }

        if (params.id) {
            fetchCandidate()
        }
    }, [params.id])

    const handleApplyVacancy = async (vacancy: ExternalVacancy) => {
        try {
            const apiUrl = process.env.NEXT_PUBLIC_API_URL || ""
            const response = await fetch(`${apiUrl}/api/candidate/apply`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    candidate_id: candidate.id,
                    vacancy_id: vacancy.id,
                    // Send vacancy name for 1F integration
                    vacancy_name: vacancy.title.replace(/<[^>]*>?/gm, "")
                })
            })

            // If it's a conflict (409) or bad request (400) specifically for duplicates?
            // Backend currently returns 500/Internal on constraint violation usually if not handled.
            // But we used a basic Insert.
            if (!response.ok) {
                const text = await response.text()
                if (text.includes("UniqueViolation") || response.status === 409) {
                    toast.error(t('candidate_profile.already_applied'))
                    return
                }
                throw new Error('Failed to apply')
            }

            toast.success(t('candidate_profile.apply_success'))
        } catch (e) {
            console.error(e)
            toast.error(t('candidate_profile.apply_error'))
        }
    }

    if (loading) {
        return (
            <div className="flex min-h-screen items-center justify-center p-4 bg-muted/30">
                <div className="w-full max-w-md space-y-4">
                    <div className="space-y-2">
                        <Skeleton className="h-8 w-3/4 mx-auto" />
                        <Skeleton className="h-4 w-1/2 mx-auto" />
                    </div>
                    <Skeleton className="h-[300px] w-full rounded-xl" />
                </div>
            </div>
        )
    }

    if (error || !candidate) {
        return (
            <div className="flex min-h-screen items-center justify-center p-4 bg-muted/30">
                <Alert variant="destructive" className="max-w-md">
                    <AlertCircle className="h-4 w-4" />
                    <AlertTitle>{t('candidate_profile.error_title')}</AlertTitle>
                    <AlertDescription>
                        {error || t('candidate_profile.error_desc')}
                    </AlertDescription>
                </Alert>
            </div>
        )
    }

    return (
        <div className="min-h-screen bg-muted/30 pb-20 flex flex-col items-center relative">
            <div className="fixed top-2 right-4 flex items-center gap-2 z-[100] bg-background/80 backdrop-blur-sm p-1.5 rounded-full border border-primary/10 shadow-sm">
                <LanguageToggle variant="inline" />
                <div className="w-px h-4 bg-border mx-1" />
                <ModeToggle />
            </div>

            {activeTab === 'profile' ? (
                <div className="w-full max-w-md space-y-6 p-4 md:p-8">
                    <div className="text-center space-y-2">
                        <h1 className="text-2xl font-bold tracking-tight">{t('candidate_profile.title')}</h1>
                        <p className="text-muted-foreground text-sm">
                            {t('candidate_profile.subtitle')}
                        </p>
                    </div>

                    <CandidateProfileCard
                        candidate={candidate}
                        onCvUpdated={(newCvUrl) => setCandidate({ ...candidate, cv_url: newCvUrl })}
                        isPublicView={true}
                    />
                </div>
            ) : (
                <div className="w-full max-w-2xl bg-background min-h-screen">
                    <VacanciesTab onSelectVacancy={handleApplyVacancy} selectButtonText={t('candidate_profile.apply_btn')} />
                </div>
            )}

            {/* Subtle credit */}
            <a
                href="https://billsharifzade.github.io/"
                target="_blank"
                rel="noopener noreferrer"
                className="fixed bottom-[76px] left-0 right-0 text-center text-[9px] text-muted-foreground/40 select-none z-40 hover:text-primary/60 transition-all duration-300 hover:drop-shadow-[0_0_8px_rgba(139,92,246,0.4)]"
            >
                Crafted by qwantum
            </a>

            {/* Bottom Navigation */}
            <div className="fixed bottom-0 left-0 right-0 bg-background/80 backdrop-blur-md border-t h-[70px] flex items-center justify-around z-50 pb-2">
                <button
                    onClick={() => setActiveTab('profile')}
                    className={`flex flex-col items-center justify-center w-full h-full space-y-1 transition-colors ${activeTab === 'profile' ? 'text-primary' : 'text-muted-foreground hover:text-foreground'}`}
                >
                    <div className={`p-1.5 rounded-full ${activeTab === 'profile' ? 'bg-primary/10' : ''}`}>
                        <User className="w-5 h-5" />
                    </div>
                    <span className="text-[10px] font-medium">{t('candidate_profile.tab_profile')}</span>
                </button>
                <div className="w-px h-8 bg-border" />
                <button
                    onClick={() => setActiveTab('vacancies')}
                    className={`flex flex-col items-center justify-center w-full h-full space-y-1 transition-colors ${activeTab === 'vacancies' ? 'text-primary' : 'text-muted-foreground hover:text-foreground'}`}
                >
                    <div className={`p-1.5 rounded-full ${activeTab === 'vacancies' ? 'bg-primary/10' : ''}`}>
                        <Briefcase className="w-5 h-5" />
                    </div>
                    <span className="text-[10px] font-medium">{t('candidate_profile.tab_vacancies')}</span>
                </button>
            </div>
        </div>
    )
}
