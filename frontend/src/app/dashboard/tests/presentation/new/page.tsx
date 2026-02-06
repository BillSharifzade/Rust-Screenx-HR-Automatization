'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from '@/lib/api';
import { CreateTestPayload } from '@/types/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Card, CardContent, CardHeader, CardTitle, CardDescription, CardFooter } from '@/components/ui/card';
import { ArrowLeft, Plus, Trash2, HelpCircle } from 'lucide-react';
import { toast } from 'sonner';
import Link from 'next/link';
import { useTranslation } from '@/lib/i18n-context';
import {
    Tooltip,
    TooltipContent,
    TooltipProvider,
    TooltipTrigger,
} from "@/components/ui/tooltip";

export default function NewPresentationPage() {
    const { t } = useTranslation();
    const router = useRouter();
    const queryClient = useQueryClient();

    const [title, setTitle] = useState('');
    const [description, setDescription] = useState('');
    const [instructions, setInstructions] = useState('');
    const [deadlineHours, setDeadlineHours] = useState(48); // default 48 hours
    const [passingScore, setPassingScore] = useState(70); // default 70%
    const [themes, setThemes] = useState<string[]>(['']);
    const [extraInfo, setExtraInfo] = useState('');

    const createMutation = useMutation({
        mutationFn: (payload: CreateTestPayload) => apiFetch('/api/integration/tests', {
            method: 'POST',
            body: JSON.stringify(payload),
        }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ['tests'] });
            toast.success(t('dashboard.tests_new.toasts.success'));
            router.push('/dashboard/tests');
        },
        onError: (err) => {
            toast.error(`${t('dashboard.tests_new.toasts.error')}: ${err.message}`);
        },
    });

    const handleAddTheme = () => {
        setThemes([...themes, '']);
    };

    const handleRemoveTheme = (index: number) => {
        const newThemes = [...themes];
        newThemes.splice(index, 1);
        setThemes(newThemes);
    };

    const handleThemeChange = (index: number, value: string) => {
        const newThemes = [...themes];
        newThemes[index] = value;
        setThemes(newThemes);
    };

    const handleSubmit = () => {
        if (!title.trim()) {
            toast.error(t('dashboard.tests_new.toasts.error'));
            return;
        }

        const validThemes = themes.filter(t => t.trim() !== '');

        const payload: CreateTestPayload = {
            title,
            description,
            instructions,
            duration_minutes: deadlineHours * 60, // Convert hours to minutes
            passing_score: passingScore,
            questions: [], // No questions
            test_type: 'presentation',
            presentation_themes: validThemes,
            presentation_extra_info: extraInfo,
            show_results_immediately: false,
            shuffle_questions: false,
            shuffle_options: false
        };

        createMutation.mutate(payload);
    };

    return (
        <div className="max-w-4xl mx-auto space-y-6">
            <div className="flex items-center gap-4">
                <Link href="/dashboard/tests">
                    <Button variant="ghost" size="icon">
                        <ArrowLeft className="h-4 w-4" />
                    </Button>
                </Link>
                <div>
                    <h3 className="text-2xl font-bold tracking-tight">{t('dashboard.attempts.presentation.create_title')}</h3>
                    <p className="text-muted-foreground">{t('dashboard.attempts.presentation.create_subtitle')}</p>
                </div>
            </div>

            <Card>
                <CardHeader>
                    <CardTitle>{t('dashboard.tests_new.details_title')}</CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                    <div className="space-y-2">
                        <Label htmlFor="title">{t('dashboard.tests_new.labels.title')}</Label>
                        <Input
                            id="title"
                            placeholder={t('dashboard.tests_new.placeholders.title')}
                            value={title}
                            onChange={(e) => setTitle(e.target.value)}
                        />
                    </div>

                    <div className="space-y-2">
                        <Label htmlFor="description">{t('dashboard.tests_new.labels.description')}</Label>
                        <Textarea
                            id="description"
                            placeholder={t('dashboard.tests_new.placeholders.description')}
                            value={description}
                            onChange={(e) => setDescription(e.target.value)}
                        />
                    </div>

                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                        <div className="space-y-2">
                            <Label htmlFor="deadline" className="flex items-center gap-2">
                                {t('dashboard.attempts.presentation.deadline_hours')}
                                <TooltipProvider>
                                    <Tooltip>
                                        <TooltipTrigger>
                                            <HelpCircle className="h-4 w-4 text-muted-foreground" />
                                        </TooltipTrigger>
                                        <TooltipContent>
                                            <p>{t('dashboard.attempts.presentation.deadline_desc')}</p>
                                        </TooltipContent>
                                    </Tooltip>
                                </TooltipProvider>
                            </Label>
                            <Input
                                id="deadline"
                                type="number"
                                min="1"
                                value={deadlineHours}
                                onChange={(e) => setDeadlineHours(parseInt(e.target.value) || 0)}
                            />
                        </div>

                        <div className="space-y-2">
                            <Label htmlFor="passing_score">{t('dashboard.tests_new.labels.passing_score')}</Label>
                            <Input
                                id="passing_score"
                                type="number"
                                min="0"
                                max="100"
                                value={passingScore}
                                onChange={(e) => setPassingScore(parseInt(e.target.value) || 0)}
                            />
                        </div>
                    </div>
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>{t('dashboard.attempts.presentation.themes')}</CardTitle>
                    <CardDescription>{t('dashboard.attempts.presentation.themes_desc')}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    {themes.map((theme, index) => (
                        <div key={index} className="flex gap-2">
                            <Input
                                value={theme}
                                onChange={(e) => handleThemeChange(index, e.target.value)}
                                placeholder={`${t('dashboard.attempts.presentation.theme_placeholder')} ${index + 1}`}
                            />
                            <Button
                                variant="ghost"
                                size="icon"
                                onClick={() => handleRemoveTheme(index)}
                                disabled={themes.length === 1}
                            >
                                <Trash2 className="h-4 w-4 text-destructive" />
                            </Button>
                        </div>
                    ))}
                    <Button variant="outline" onClick={handleAddTheme} className="w-full">
                        <Plus className="mr-2 h-4 w-4" />
                        {t('dashboard.attempts.presentation.add_theme')}
                    </Button>
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>{t('dashboard.attempts.presentation.extra_info')}</CardTitle>
                </CardHeader>
                <CardContent>
                    <Textarea
                        placeholder={t('dashboard.attempts.presentation.extra_info_placeholder')}
                        value={extraInfo}
                        onChange={(e) => setExtraInfo(e.target.value)}
                        className="min-h-[150px]"
                    />
                </CardContent>
                <CardFooter className="flex justify-end gap-2">
                    <Link href="/dashboard/tests">
                        <Button variant="outline">{t('dashboard.tests_new.actions.cancel')}</Button>
                    </Link>
                    <Button onClick={handleSubmit} disabled={createMutation.isPending}>
                        {createMutation.isPending ? t('dashboard.tests_new.actions.creating') : t('dashboard.tests_new.actions.create')}
                    </Button>
                </CardFooter>
            </Card>
        </div>
    );
}
