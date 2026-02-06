'use client';

import { useState, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { useRouter } from 'next/navigation';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from '@/lib/api';
import { CreateTestPayload } from '@/types/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { toast } from 'sonner';
import { Plus, Trash2, Sparkles, ArrowLeft } from 'lucide-react';
import { Textarea } from '@/components/ui/textarea';
import { useTranslation } from '@/lib/i18n-context';
import { motion, AnimatePresence } from 'framer-motion';
import Link from 'next/link';

export default function NewTestPage() {
    const { t } = useTranslation();
    const router = useRouter();
    const queryClient = useQueryClient();
    const [showAIDialog, setShowAIDialog] = useState(false);
    const [aiPrompt, setAiPrompt] = useState('');
    const [aiProfession, setAiProfession] = useState('');
    const [aiNumQuestions, setAiNumQuestions] = useState(10);
    const [mounted, setMounted] = useState(false);

    useEffect(() => {
        setMounted(true);
    }, []);

    const [formData, setFormData] = useState<CreateTestPayload>({
        title: '',
        description: '',
        instructions: '',
        duration_minutes: 30,
        passing_score: 70,
        questions: [],
    });

    const generateId = () => Math.random().toString(36).substr(2, 9);
    const [animBaseIdx, setAnimBaseIdx] = useState(0);

    const [isCreating, setIsCreating] = useState(false);

    const createMutation = useMutation({
        mutationFn: (data: CreateTestPayload) =>
            apiFetch('/api/integration/tests', {
                method: 'POST',
                body: JSON.stringify(data),
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ['tests'] });
            toast.success(t('dashboard.tests_new.toasts.success'));
            setIsCreating(true);
            setTimeout(() => {
                router.push('/dashboard/tests');
            }, 1500);
        },
        onError: (error) => {
            toast.error(`${t('dashboard.tests_new.toasts.error')}: ${error.message}`);
        },
    });

    const aiGenerateMutation = useMutation({
        mutationFn: (data: { prompt: string; profession: string; num_questions: number }) =>
            apiFetch<any>('/api/integration/tests/generate-ai', {
                method: 'POST',
                body: JSON.stringify({
                    profession: data.profession,
                    description: data.prompt,
                    num_questions: data.num_questions,
                    persist: false,
                }),
            }),
        onSuccess: (data) => {
            if (data.questions || data.test) {
                const rawQuestions = data.questions || data.test?.questions || [];
                const questions = rawQuestions.map((q: any) => ({ ...q, _uid: generateId() }));
                const title = data.test?.title || formData.title;
                const description = data.test?.description || formData.description;

                setAnimBaseIdx(formData.questions?.length || 0);
                setFormData({
                    ...formData,
                    title,
                    description,
                    questions: [...(formData.questions || []), ...questions],
                });
                setAiNumQuestions(10);
            }
        },
        onError: (error) => {
            toast.error(`${t('dashboard.tests_new.toasts.ai_error')}: ${error.message}`);
        },
    });

    const scrollToQuestion = (index: number) => {
        setTimeout(() => {
            const element = document.getElementById(`question-card-${index}`);
            if (element) {
                element.scrollIntoView({ behavior: 'smooth', block: 'center' }); // center frames the card better
            }
        }, 100);
    };

    const handleAIGenerate = () => {
        if (!aiProfession.trim()) {
            toast.error(t('dashboard.tests_new.toasts.profession_required'));
            return;
        }
        aiGenerateMutation.mutate({ prompt: aiPrompt, profession: aiProfession, num_questions: aiNumQuestions });
    };



    const addQuestion = () => {
        setAnimBaseIdx(formData.questions?.length || 0);
        setFormData({
            ...formData,
            questions: [
                ...(formData.questions || []),
                {
                    _uid: generateId(),
                    type: 'multiple_choice',
                    question: '',
                    points: 10,
                    options: ['', '', '', ''],
                    correct_answer: '',
                    min_words: 20,
                    expected_keywords: [],
                },
            ],
        });
    };

    const removeQuestion = (index: number) => {
        const newQuestions = [...(formData.questions || [])];
        newQuestions.splice(index, 1);
        setFormData({ ...formData, questions: newQuestions });
    };

    const updateQuestion = (index: number, field: string, value: any) => {
        const newQuestions = [...(formData.questions || [])];
        newQuestions[index] = { ...newQuestions[index], [field]: value };
        setFormData({ ...formData, questions: newQuestions });
    };

    const handleSubmit = (e: React.FormEvent) => {
        e.preventDefault();
        createMutation.mutate(formData);
    };

    if (isCreating) {
        return (
            <div className="fixed inset-0 z-[9999] bg-background/95 backdrop-blur-md flex items-center justify-center">
                <motion.div
                    initial={{ opacity: 0, scale: 0.8 }}
                    animate={{ opacity: 1, scale: 1 }}
                    className="flex flex-col items-center gap-6 text-center p-4"
                >
                    <motion.div
                        animate={{ scale: [1, 1.1, 1] }}
                        transition={{ duration: 1, repeat: Infinity }}
                        className="h-20 w-20 rounded-full bg-green-500/20 flex items-center justify-center"
                    >
                        <motion.div
                            initial={{ scale: 0 }}
                            animate={{ scale: 1 }}
                            transition={{ delay: 0.2, type: "spring", bounce: 0.5 }}
                        >
                            <Plus className="h-10 w-10 text-green-500" />
                        </motion.div>
                    </motion.div>
                    <div className="space-y-2">
                        <h3 className="text-xl font-semibold">{t('dashboard.tests_new.toasts.success')}</h3>
                        <p className="text-muted-foreground">{t('common.redirecting') || 'Redirecting...'}</p>
                    </div>
                </motion.div>
            </div>
        );
    }

    return (
        <div className="max-w-4xl space-y-6">
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                    <Link href="/dashboard/tests">
                        <Button variant="ghost" size="icon" type="button">
                            <ArrowLeft className="h-4 w-4" />
                        </Button>
                    </Link>
                    <div className="space-y-1">
                        <h3 className="text-2xl font-bold tracking-tight">{t('dashboard.tests_new.title')}</h3>
                        <p className="text-muted-foreground">
                            {t('dashboard.tests_new.subtitle')}
                        </p>
                    </div>
                </div>
                <Button
                    type="button"
                    onClick={() => setShowAIDialog(!showAIDialog)}
                    variant="outline"
                    className="gap-2"
                >
                    <Sparkles className="h-4 w-4" />
                    {t('dashboard.tests_new.ai_btn')}
                </Button>
            </div>

            {showAIDialog && (
                <Card className="border-primary/50 bg-primary/5">
                    <CardHeader>
                        <CardTitle className="flex items-center gap-2">
                            <Sparkles className="h-5 w-5 text-primary" />
                            {t('dashboard.tests_new.ai_dialog.title')}
                        </CardTitle>
                        <CardDescription>
                            {t('dashboard.tests_new.ai_dialog.desc')}
                        </CardDescription>
                    </CardHeader>
                    <CardContent className="space-y-4">
                        <div className="space-y-2">
                            <Label htmlFor="ai-profession">{t('dashboard.tests_new.ai_dialog.profession')} <span className="text-destructive">*</span></Label>
                            <Input
                                id="ai-profession"
                                placeholder={t('dashboard.tests_new.ai_dialog.profession_placeholder')}
                                value={aiProfession}
                                onChange={(e) => setAiProfession(e.target.value)}
                            />
                        </div>
                        <div className="space-y-2">
                            <Label htmlFor="ai-prompt">{t('dashboard.tests_new.ai_dialog.context')}</Label>
                            <Textarea
                                id="ai-prompt"
                                placeholder={t('dashboard.tests_new.ai_dialog.prompt_placeholder')}
                                value={aiPrompt}
                                onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) => setAiPrompt(e.target.value)}
                                rows={3}
                            />
                        </div>
                        <div className="space-y-2">
                            <Label htmlFor="ai-num-questions">{t('dashboard.tests_new.ai_dialog.num_questions')}</Label>
                            <Input
                                id="ai-num-questions"
                                type="number"
                                min="1"
                                max="25"
                                value={aiNumQuestions}
                                onChange={(e) => setAiNumQuestions(Math.min(25, Math.max(1, parseInt(e.target.value) || 1)))}
                            />
                        </div>
                        <div className="flex gap-2">
                            <Button
                                onClick={handleAIGenerate}
                                disabled={aiGenerateMutation.isPending}
                                className="gap-2"
                            >
                                <Sparkles className="h-4 w-4" />
                                {t('dashboard.tests_new.ai_dialog.submit')}
                            </Button>
                            <Button
                                variant="outline"
                                onClick={() => {
                                    setShowAIDialog(false);
                                    setAiPrompt('');
                                    setAiProfession('');
                                }}
                            >
                                {t('common.cancel')}
                            </Button>
                        </div>
                    </CardContent>
                </Card>
            )}

            {mounted && createPortal(
                <AnimatePresence mode="wait">
                    {aiGenerateMutation.isPending && (
                        <motion.div
                            key="loader"
                            initial={{ opacity: 0 }}
                            animate={{ opacity: 1 }}
                            exit={{ opacity: 0, transition: { duration: 0.5, ease: "easeInOut" } }}
                            className="fixed inset-0 z-[9999] bg-background/80 backdrop-blur-md flex items-center justify-center w-screen h-screen top-0 left-0"
                        >
                            <div className="flex flex-col items-center gap-6 text-center p-4 max-w-lg">
                                <div className="relative">
                                    <motion.div
                                        animate={{
                                            scale: [1, 1.2, 1],
                                            opacity: [0.5, 0.8, 0.5],
                                        }}
                                        transition={{
                                            duration: 2,
                                            repeat: Infinity,
                                            ease: "easeInOut",
                                        }}
                                        className="absolute inset-0 bg-primary/30 blur-2xl rounded-full"
                                    />
                                    <Sparkles className="h-16 w-16 text-primary relative z-10" />
                                </div>
                                <motion.h3
                                    animate={{ opacity: [0.5, 1, 0.5] }}
                                    transition={{ duration: 3, repeat: Infinity, ease: "easeInOut" }}
                                    className="text-xl font-medium tracking-wide"
                                >
                                    {t('dashboard.tests_new.ai_dialog.generating')}
                                </motion.h3>
                            </div>
                        </motion.div>
                    )}
                </AnimatePresence>,
                document.body
            )}

            <form onSubmit={handleSubmit} className="space-y-6">
                <Card>
                    <CardHeader>
                        <CardTitle>{t('dashboard.tests_new.details_title')}</CardTitle>
                    </CardHeader>
                    <CardContent className="space-y-4">
                        <div className="space-y-2">
                            <Label htmlFor="title">{t('dashboard.tests_new.labels.title')}</Label>
                            <Input
                                id="title"
                                required
                                value={formData.title}
                                onChange={(e: React.ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, title: e.target.value })}
                                placeholder={t('dashboard.tests_new.labels.title_placeholder')}
                            />
                        </div>
                        <div className="space-y-2">
                            <Label htmlFor="description">{t('dashboard.tests_new.labels.description')}</Label>
                            <Textarea
                                id="description"
                                value={formData.description}
                                onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) => setFormData({ ...formData, description: e.target.value })}
                                placeholder={t('dashboard.tests_new.labels.description_placeholder')}
                                rows={3}
                            />
                        </div>
                        <div className="grid grid-cols-2 gap-4">
                            <div className="space-y-2">
                                <Label htmlFor="duration">{t('dashboard.tests_new.labels.duration')}</Label>
                                <Input
                                    id="duration"
                                    type="number"
                                    required
                                    min="1"
                                    value={formData.duration_minutes}
                                    onChange={(e: React.ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, duration_minutes: parseInt(e.target.value) })}
                                />
                            </div>
                            <div className="space-y-2">
                                <Label htmlFor="passing_score">{t('dashboard.tests_new.labels.passing_score')}</Label>
                                <Input
                                    id="passing_score"
                                    type="number"
                                    required
                                    min="0"
                                    max="100"
                                    value={formData.passing_score}
                                    onChange={(e) => setFormData({ ...formData, passing_score: parseInt(e.target.value) })}
                                />
                            </div>
                        </div>
                    </CardContent>
                </Card>

                <div className="space-y-4">
                    <div className="flex items-center justify-between">
                        <div>
                            <h4 className="text-lg font-medium">{t('dashboard.tests_new.questions_list.title')}</h4>
                            <p className="text-sm text-muted-foreground">
                                {formData.questions?.length || 0} {t('dashboard.tests_new.questions_list.count')}
                            </p>
                        </div>
                        <Button type="button" onClick={addQuestion} variant="outline" size="sm">
                            <Plus className="mr-2 h-4 w-4" />
                            {t('dashboard.tests_new.questions_list.add_btn')}
                        </Button>
                    </div>

                    {(!formData.questions || formData.questions.length === 0) && (
                        <Card className="border-dashed">
                            <CardContent className="flex flex-col items-center justify-center h-32 text-center">
                                <p className="text-muted-foreground">{t('dashboard.tests_new.questions_list.empty')}</p>
                                <p className="text-sm text-muted-foreground/70 mt-1">
                                    {t('dashboard.tests_new.questions_list.empty_desc')}
                                </p>
                            </CardContent>
                        </Card>
                    )}

                    <div className="grid gap-4">
                        <AnimatePresence mode="popLayout">
                            {formData.questions?.map((question, index) => (
                                <motion.div
                                    id={`question-card-${index}`}
                                    key={question._uid || index}
                                    layout
                                    initial={{ opacity: 0, y: 50, scale: 0.95, filter: "blur(10px)" }}
                                    animate={{
                                        opacity: 1,
                                        y: 0,
                                        scale: 1,
                                        filter: "blur(0px)",
                                        transition: {
                                            // Calculate delay relative to the start of the current batch
                                            delay: Math.max(0, index - animBaseIdx) * 0.4 + 0.3, // Slower 0.4s stagger for "card by card" feel
                                            duration: 0.5,
                                            type: "spring",
                                            bounce: 0.3
                                        }
                                    }}
                                    onAnimationComplete={() => scrollToQuestion(index)}
                                    exit={{ opacity: 0, scale: 0.9, filter: "blur(10px)", transition: { duration: 0.2 } }}
                                    className="relative group"
                                >
                                    <motion.div
                                        initial={{ opacity: 0, scale: 1.2 }}
                                        animate={{ opacity: [0, 1, 0], scale: 1 }}
                                        transition={{ duration: 0.8, delay: index * 0.15 + 0.6, ease: "easeOut" }}
                                        className="absolute inset-0 z-10 bg-primary/10 rounded-xl pointer-events-none border border-primary/50"
                                    />
                                    <Card className="premium-hover relative overflow-hidden border-primary/20">
                                        <div className="absolute top-0 right-0 p-2 opacity-50">
                                            <Sparkles className="w-3 h-3 text-primary/40" />
                                        </div>
                                        <CardContent className="pt-6">
                                            <div className="flex items-start gap-4">
                                                <div className="flex-1 space-y-4">
                                                    <div className="flex gap-4">
                                                        <div className="flex-1 space-y-2">
                                                            <Label>{t('dashboard.tests_new.question_card.question')} {index + 1}</Label>
                                                            <Textarea
                                                                value={question.question}
                                                                onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) => updateQuestion(index, 'question', e.target.value)}
                                                                placeholder={t('dashboard.tests_new.question_card.question_placeholder')}
                                                                rows={2}
                                                            />
                                                        </div>
                                                        <div className="w-48 space-y-2">
                                                            <Label>{t('dashboard.tests_new.question_card.type')}</Label>
                                                            <Select
                                                                value={question.type}
                                                                onValueChange={(value) => updateQuestion(index, 'type', value)}
                                                            >
                                                                <SelectTrigger>
                                                                    <SelectValue />
                                                                </SelectTrigger>
                                                                <SelectContent>
                                                                    <SelectItem value="multiple_choice">{t('dashboard.tests_new.question_card.types.multiple_choice')}</SelectItem>
                                                                    <SelectItem value="short_answer">{t('dashboard.tests_new.question_card.types.short_answer')}</SelectItem>
                                                                </SelectContent>
                                                            </Select>
                                                        </div>
                                                        <div className="w-24 space-y-2">
                                                            <Label>{t('dashboard.tests_new.question_card.points')}</Label>
                                                            <Input
                                                                type="number"
                                                                min="1"
                                                                value={question.points}
                                                                onChange={(e) => updateQuestion(index, 'points', parseInt(e.target.value) || 0)}
                                                            />
                                                        </div>
                                                    </div>

                                                    {question.type === 'multiple_choice' && (
                                                        <div className="space-y-4">
                                                            <div className="space-y-2">
                                                                <Label>{t('dashboard.tests_new.question_card.options')}</Label>
                                                                {question.options?.map((option: string, optIndex: number) => (
                                                                    <Input
                                                                        key={optIndex}
                                                                        value={option}
                                                                        onChange={(e) => {
                                                                            const newOptions = [...(question.options || [])];
                                                                            newOptions[optIndex] = e.target.value;
                                                                            updateQuestion(index, 'options', newOptions);
                                                                        }}
                                                                        placeholder={`${t('dashboard.tests_new.question_card.option')} ${optIndex + 1}`}
                                                                        className="mb-2"
                                                                    />
                                                                ))}
                                                            </div>
                                                            <div className="space-y-2">
                                                                <Label>{t('dashboard.tests_new.question_card.correct_answer')}</Label>
                                                                <Input
                                                                    type="number"
                                                                    min="1"
                                                                    max="4"
                                                                    value={Number(question.correct_answer || 0) + 1}
                                                                    onChange={(e) => {
                                                                        const val = parseInt(e.target.value) || 1;
                                                                        const clamped = Math.min(4, Math.max(1, val));
                                                                        updateQuestion(index, 'correct_answer', clamped - 1);
                                                                    }}
                                                                    placeholder={t('dashboard.tests_new.question_card.correct_answer_placeholder')}
                                                                />
                                                            </div>
                                                        </div>
                                                    )}

                                                    {question.type === 'short_answer' && (
                                                        <div className="space-y-4">
                                                            <div className="space-y-2">
                                                                <Label>{t('dashboard.tests_new.question_card.min_words')}</Label>
                                                                <Input
                                                                    type="number"
                                                                    min="1"
                                                                    value={question.min_words || 20}
                                                                    onChange={(e) => updateQuestion(index, 'min_words', parseInt(e.target.value) || 0)}
                                                                />
                                                            </div>
                                                            <div className="space-y-2">
                                                                <Label>{t('dashboard.tests_new.question_card.keywords')}</Label>
                                                                <Input
                                                                    value={question.expected_keywords?.join(', ') || ''}
                                                                    onChange={(e) => updateQuestion(index, 'expected_keywords', e.target.value.split(',').map(s => s.trimStart()))}
                                                                    placeholder=""
                                                                />
                                                                <p className="text-xs text-muted-foreground">
                                                                    {t('dashboard.tests_new.question_card.keywords_desc')}
                                                                </p>
                                                            </div>
                                                        </div>
                                                    )}
                                                </div>
                                                <Button
                                                    type="button"
                                                    variant="ghost"
                                                    size="icon"
                                                    className="text-destructive hover:bg-destructive/10"
                                                    onClick={() => removeQuestion(index)}
                                                >
                                                    <Trash2 className="h-4 w-4" />
                                                </Button>
                                            </div>
                                        </CardContent>
                                    </Card>
                                </motion.div>
                            ))}
                        </AnimatePresence>
                    </div>

                </div>

                <div className="flex justify-end gap-2">
                    <Button
                        type="button"
                        variant="outline"
                        onClick={() => router.back()}
                    >
                        {t('common.cancel')}
                    </Button>
                    <Button type="submit" disabled={createMutation.isPending || (formData.questions?.length ?? 0) === 0}>
                        {createMutation.isPending ? t('dashboard.tests_new.actions.creating') : t('dashboard.tests_new.actions.create')}
                    </Button>
                </div>
            </form>
        </div>
    );
}
