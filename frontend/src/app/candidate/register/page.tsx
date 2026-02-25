'use client';

import { useState, useEffect, useRef } from 'react';
import { useRouter } from 'next/navigation';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import * as z from 'zod';
import { toast } from 'sonner';
import { Upload, User, Mail, Phone, FileText, CalendarIcon, Briefcase, Check, ChevronsUpDown } from 'lucide-react';
import { motion } from 'framer-motion';
import { format } from 'date-fns';
import { useQuery } from '@tanstack/react-query';

import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from '@/components/ui/command';
import { cn } from '@/lib/utils';
import { apiFetch } from '@/lib/api';
import { ExternalVacancy, ExternalVacancyListResponse } from '@/types/api';
import { LanguageToggle } from '@/components/language-toggle';
import { ModeToggle } from '@/components/mode-toggle';
import { VacanciesTab } from '@/components/VacanciesTab';
import { useTranslation } from '@/lib/i18n-context';

// --- Types & Schema ---

const formSchema = z.object({
    name: z.string().min(2, 'Name must be at least 2 characters'),
    email: z.string().email('Invalid email address'),
    phone: z.string().min(5, 'Phone number is required'),
    vacancy_id: z.number().int().min(1, 'Please select a vacancy'),
});

type FormValues = z.infer<typeof formSchema>;

interface RegisterResponse {
    id: string;
    status: string;
}

export default function RegisterPage() {
    const { t } = useTranslation();
    const router = useRouter();
    const [activeTab, setActiveTab] = useState<'register' | 'vacancies'>('register');

    // Registration State
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [isCreating, setIsCreating] = useState(false);
    const [file, setFile] = useState<File | null>(null);
    const [telegramData, setTelegramData] = useState<any>(null);
    const [dob, setDob] = useState<Date | undefined>(undefined);
    const [dobInput, setDobInput] = useState<string>('');

    // Vacancy Selection State
    const [vacancyOpen, setVacancyOpen] = useState(false);

    // Form
    const form = useForm<FormValues>({
        resolver: zodResolver(formSchema),
        defaultValues: {
            name: '',
            email: '',
            phone: '',
        },
    });

    // Fetch vacancies for the combobox
    const { data: vacancyData } = useQuery({
        queryKey: ['external-vacancies'],
        queryFn: () => apiFetch<ExternalVacancyListResponse>('/api/external-vacancies'),
    });

    const vacancies = vacancyData?.vacancies || [];
    const hasInitialized = useRef(false);

    useEffect(() => {
        if (hasInitialized.current) return;
        hasInitialized.current = true;

        // @ts-ignore
        const tg = window?.Telegram?.WebApp;
        const searchParams = new URLSearchParams(window.location.search);

        if (tg) {
            tg.ready();
            tg.expand();
            const user = tg.initDataUnsafe?.user;
            const userId = searchParams.get('telegram_id');
            const userName = searchParams.get('name');

            if (user) {
                setTelegramData(user);
                if (user.first_name && !form.getValues('name')) {
                    let fullName = user.first_name + (user.last_name ? ` ${user.last_name}` : '');
                    form.setValue('name', fullName);
                }
            } else if (userId) {
                setTelegramData({ id: userId, first_name: userName || 'User' });
                if (userName && !form.getValues('name')) {
                    form.setValue('name', userName);
                }
            }

            // Autopaste Phone (from Telegram or URL)
            const phone = user?.phone_number || searchParams.get('phone');
            if (phone && !form.getValues('phone')) {
                form.setValue('phone', phone);
            }

            // Autopaste DOB (from URL params only — Telegram API does not expose DOB)
            const dobParam = searchParams.get('dob');
            if (dobParam && !dob) {
                const parsedDate = new Date(dobParam);
                if (!isNaN(parsedDate.getTime())) {
                    setDob(parsedDate);
                    setDobInput(format(parsedDate, 'dd.MM.yyyy'));
                }
            }
        } else {
            // Fallback for non-Telegram users (only URL params)
            const userId = searchParams.get('telegram_id');
            const userName = searchParams.get('name');

            if (userId) {
                setTelegramData({ id: userId, first_name: userName || 'User' });
            }

            if (userName && !form.getValues('name')) {
                form.setValue('name', userName);
            }
            const phone = searchParams.get('phone');
            if (phone && !form.getValues('phone')) {
                form.setValue('phone', phone);
            }

            const name = searchParams.get('name');
            if (name && !form.getValues('name')) {
                form.setValue('name', name);
            }

            const dobParam = searchParams.get('dob');
            if (dobParam && !dob) {
                const parsedDate = new Date(dobParam);
                if (!isNaN(parsedDate.getTime())) {
                    setDob(parsedDate);
                    setDobInput(format(parsedDate, 'dd.MM.yyyy'));
                }
            }
        }
    }, [form]);

    const handleSelectVacancyFromTab = (vacancy: ExternalVacancy) => {
        form.setValue('vacancy_id', vacancy.id);
        setActiveTab('register');
        toast.success(`${t('common.success')}: ${vacancy.title.replace(/<[^>]*>?/gm, '')}`);
    };

    const handleDobChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        let val = e.target.value.replace(/\D/g, ''); // Remove non-digits
        if (val.length > 8) val = val.slice(0, 8); // Max 8 digits

        // Formatting as DD.MM.YYYY
        if (val.length >= 5) {
            val = `${val.slice(0, 2)}.${val.slice(2, 4)}.${val.slice(4)}`;
        } else if (val.length >= 3) {
            val = `${val.slice(0, 2)}.${val.slice(2)}`;
        }

        setDobInput(val);

        if (val.length === 10) {
            const [day, month, year] = val.split('.');
            const parsedDate = new Date(`${year}-${month}-${day}`);
            const currentYear = new Date().getFullYear();
            if (
                !isNaN(parsedDate.getTime()) &&
                parsedDate.getFullYear() > 1900 &&
                parsedDate.getFullYear() <= currentYear - 16 &&
                parsedDate.getDate() === parseInt(day, 10)
            ) {
                setDob(parsedDate);
            } else {
                setDob(undefined);
            }
        } else {
            setDob(undefined);
        }
    };

    const onSubmit = async (values: FormValues) => {
        let hasError = false;
        if (!file) {
            toast.error(t('registration.cv_required'));
            hasError = true;
        }
        if (!dob) {
            toast.error(t('registration.dob_required'));
            hasError = true;
        }

        if (hasError) return;

        setIsSubmitting(true);

        try {
            const formData = new FormData();
            formData.append('name', values.name);
            formData.append('email', values.email);
            if (values.phone) formData.append('phone', values.phone);

            const tid = telegramData?.id?.toString();
            if (tid) {
                formData.append('telegram_id', tid);
            }

            if (dob) formData.append('dob', format(dob, 'yyyy-MM-dd'));
            if (values.vacancy_id) formData.append('vacancy_id', values.vacancy_id.toString());
            formData.append('cv', file!);

            const profileData = JSON.stringify({
                telegram_username: telegramData?.username,
                registration_date: new Date().toISOString(),
            });
            formData.append('profile_data', profileData);

            const response = await fetch(`${process.env.NEXT_PUBLIC_API_URL || ''}/api/candidate/register`, {
                method: 'POST',
                body: formData,
            });

            if (!response.ok) {
                const errorData = await response.json();
                throw new Error(errorData.error || t('common.error'));
            }

            const data: RegisterResponse = await response.json();

            setIsSubmitting(false);
            setIsCreating(true);

            await new Promise((resolve) => setTimeout(resolve, 2000));

            router.push(`/candidate/${data.id}`);
        } catch (error) {
            console.error(error);
            toast.error(error instanceof Error ? error.message : t('common.error'));
            setIsSubmitting(false);
        }
    };

    if (isCreating) {
        return (
            <div className="flex min-h-screen flex-col items-center justify-center p-4 bg-background">
                <motion.div
                    initial={{ opacity: 0, scale: 0.9 }}
                    animate={{ opacity: 1, scale: 1 }}
                    className="flex flex-col items-center text-center space-y-6"
                >
                    <div className="relative">
                        <div className="h-24 w-24 rounded-full border-4 border-primary/20 border-t-primary animate-spin" />
                        <div className="absolute inset-0 flex items-center justify-center">
                            <User className="h-8 w-8 text-primary animate-pulse" />
                        </div>
                    </div>
                    <div className="space-y-2">
                        <h1 className="text-2xl font-bold">{t('common.loading')}</h1>
                        <p className="text-muted-foreground max-w-xs">{t('registration.subtitle')}</p>
                    </div>
                </motion.div>
            </div>
        );
    }

    return (
        <div className="min-h-screen bg-background text-foreground pb-20 relative">
            <div className="fixed top-2 right-4 flex items-center gap-2 z-[100] bg-background/80 backdrop-blur-sm p-1.5 rounded-full border border-primary/10 shadow-sm">
                <LanguageToggle variant="inline" />
                <div className="w-px h-4 bg-border mx-1" />
                <ModeToggle />
            </div>
            {activeTab === 'register' ? (
                <div className="flex items-center justify-center p-4 min-h-[85vh]">
                    <div className="w-full max-w-md space-y-6">
                        <div className="space-y-2 text-center">
                            <div className="mx-auto w-12 h-12 bg-primary/10 rounded-full flex items-center justify-center mb-4">
                                <User className="w-6 h-6 text-primary" />
                            </div>
                            <h1 className="text-3xl font-bold">{t('registration.title')}</h1>
                            <p className="text-muted-foreground">{t('registration.subtitle')}</p>
                        </div>

                        <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-6">
                            <div className="space-y-4">
                                <div className="space-y-2">
                                    <Label htmlFor="name">{t('registration.full_name')} <span className="text-destructive">*</span></Label>
                                    <div className="relative">
                                        <User className="absolute left-3 top-3 h-4 w-4 text-muted-foreground" />
                                        <Input
                                            id="name"
                                            placeholder="John Doe"
                                            className="pl-9 h-11"
                                            {...form.register('name')}
                                        />
                                    </div>
                                    {form.formState.errors.name && (
                                        <p className="text-xs text-destructive">{form.formState.errors.name.message}</p>
                                    )}
                                </div>

                                <div className="space-y-2">
                                    <Label htmlFor="email">Email <span className="text-destructive">*</span></Label>
                                    <div className="relative">
                                        <Mail className="absolute left-3 top-3 h-4 w-4 text-muted-foreground" />
                                        <Input
                                            id="email"
                                            type="email"
                                            placeholder="ivan@example.com"
                                            className="pl-9 h-11"
                                            {...form.register('email')}
                                        />
                                    </div>
                                    {form.formState.errors.email && (
                                        <p className="text-xs text-destructive">{form.formState.errors.email.message}</p>
                                    )}
                                </div>

                                <div className="space-y-2">
                                    <Label htmlFor="phone">{t('registration.phone')} <span className="text-destructive">*</span></Label>
                                    <div className="relative">
                                        <Phone className="absolute left-3 top-3 h-4 w-4 text-muted-foreground" />
                                        <Input
                                            id="phone"
                                            placeholder="+992 000 000 000"
                                            className="pl-9 h-11"
                                            {...form.register('phone')}
                                        />
                                    </div>
                                    {form.formState.errors.phone && (
                                        <p className="text-xs text-destructive">{form.formState.errors.phone.message}</p>
                                    )}
                                </div>

                                <div className="space-y-2 flex flex-col">
                                    <Label>{t('registration.select_vacancy')} <span className="text-destructive">*</span></Label>
                                    <Popover open={vacancyOpen} onOpenChange={setVacancyOpen}>
                                        <PopoverTrigger asChild>
                                            <Button
                                                variant="outline"
                                                role="combobox"
                                                aria-expanded={vacancyOpen}
                                                className="w-full justify-between h-11 pl-3 font-normal"
                                            >
                                                <span className="truncate flex-1 text-left">
                                                    {form.watch('vacancy_id')
                                                        ? vacancies
                                                            .find((vacancy) => vacancy.id === form.watch('vacancy_id'))
                                                            ?.title.replace(/<[^>]*>?/gm, '')
                                                        : t('registration.select_vacancy') + '...'}
                                                </span>
                                                <ChevronsUpDown className="ml-2 h-4 w-4 shrink-0 opacity-50" />
                                            </Button>
                                        </PopoverTrigger>
                                        <PopoverContent className="w-[350px] p-0" align="start">
                                            <Command>
                                                <CommandInput placeholder={t('common.search') + '...'} />
                                                <CommandList>
                                                    <CommandEmpty>{t('dashboard.vacancies.no_vacancies')}</CommandEmpty>
                                                    <CommandGroup>
                                                        {vacancies.map((vacancy) => (
                                                            <CommandItem
                                                                key={vacancy.id}
                                                                value={vacancy.title + ' ' + vacancy.id}
                                                                onSelect={() => {
                                                                    form.setValue('vacancy_id', vacancy.id);
                                                                    setVacancyOpen(false);
                                                                }}
                                                            >
                                                                <Check
                                                                    className={cn(
                                                                        'mr-2 h-4 w-4',
                                                                        form.watch('vacancy_id') === vacancy.id
                                                                            ? 'opacity-100'
                                                                            : 'opacity-0'
                                                                    )}
                                                                />
                                                                <span
                                                                    dangerouslySetInnerHTML={{ __html: vacancy.title }}
                                                                />
                                                            </CommandItem>
                                                        ))}
                                                    </CommandGroup>
                                                </CommandList>
                                            </Command>
                                        </PopoverContent>
                                    </Popover>
                                    {form.formState.errors.vacancy_id && (
                                        <p className="text-xs text-destructive">Please select a vacancy</p>
                                    )}
                                </div>

                                <div className="space-y-2 text-left">
                                    <Label htmlFor="dob">{t('registration.dob')} <span className="text-destructive">*</span></Label>
                                    <div className="relative">
                                        <CalendarIcon className="absolute left-3 top-3 h-4 w-4 text-muted-foreground" />
                                        <Input
                                            id="dob"
                                            type="text"
                                            inputMode="numeric"
                                            placeholder="DD.MM.YYYY"
                                            className="pl-9 h-11 transition-all"
                                            value={dobInput}
                                            onChange={handleDobChange}
                                        />
                                    </div>
                                    {!dob && dobInput.length === 10 && (
                                        <p className="text-xs text-amber-500">Please enter a valid date (min age 16)</p>
                                    )}
                                    {!dob && form.formState.isSubmitted && dobInput.length !== 10 && (
                                        <p className="text-xs text-destructive">Date of birth is required</p>
                                    )}
                                </div>

                                <div className="space-y-2 text-left">
                                    <Label>{t('registration.cv_upload')} <span className="text-destructive">*</span></Label>
                                    <div
                                        className={`border-2 border-dashed rounded-xl p-6 flex flex-col items-center justify-center transition-colors cursor-pointer ${file
                                            ? 'border-primary bg-primary/5'
                                            : 'border-muted-foreground/20 hover:border-primary/50'
                                            }`}
                                        onClick={() => document.getElementById('cv-upload')?.click()}
                                    >
                                        <input
                                            id="cv-upload"
                                            type="file"
                                            className="hidden"
                                            accept=".pdf,.docx,.doc,.png,.jpg,.jpeg,.webp"
                                            onChange={(e) => {
                                                const files = e.target.files;
                                                if (files && files[0]) setFile(files[0]);
                                            }}
                                        />
                                        {file ? (
                                            <div className="flex items-center gap-3">
                                                <FileText className="h-8 w-8 text-primary" />
                                                <div className="text-sm text-center">
                                                    <p className="font-medium truncate max-w-[180px]">{file.name}</p>
                                                    <p className="text-xs text-muted-foreground">
                                                        {(file.size / 1024 / 1024).toFixed(2)} MB
                                                    </p>
                                                </div>
                                            </div>
                                        ) : (
                                            <>
                                                <Upload className="h-8 w-8 text-muted-foreground mb-2" />
                                                <p className="text-sm font-medium">{t('registration.cv_upload')}</p>
                                                <p className="text-xs text-muted-foreground">PDF, Word, Images</p>
                                            </>
                                        )}
                                    </div>
                                    {!file && form.formState.isSubmitted && (
                                        <p className="text-xs text-destructive">CV file is required</p>
                                    )}
                                </div>
                            </div>

                            <Button
                                type="submit"
                                className="w-full h-12 font-medium text-base rounded-xl"
                                disabled={isSubmitting}
                            >
                                {isSubmitting ? t('common.loading') : t('registration.submit')}
                            </Button>
                        </form>
                    </div>
                </div>
            ) : (
                <VacanciesTab onSelectVacancy={handleSelectVacancyFromTab} />
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
                    onClick={() => setActiveTab('register')}
                    className={`flex flex-col items-center justify-center w-full h-full space-y-1 transition-colors ${activeTab === 'register' ? 'text-primary' : 'text-muted-foreground hover:text-foreground'
                        }`}
                >
                    <div
                        className={`p-1.5 rounded-full ${activeTab === 'register' ? 'bg-primary/10' : ''}`}
                    >
                        <User className="w-5 h-5" />
                    </div>
                    <span className="text-[10px] font-medium">{t('common.actions')}</span>
                </button>
                <div className="w-px h-8 bg-border" />
                <button
                    onClick={() => setActiveTab('vacancies')}
                    className={`flex flex-col items-center justify-center w-full h-full space-y-1 transition-colors ${activeTab === 'vacancies' ? 'text-primary' : 'text-muted-foreground hover:text-foreground'
                        }`}
                >
                    <div
                        className={`p-1.5 rounded-full ${activeTab === 'vacancies' ? 'bg-primary/10' : ''}`}
                    >
                        <Briefcase className="w-5 h-5" />
                    </div>
                    <span className="text-[10px] font-medium">{t('dashboard.vacancies.title')}</span>
                </button>
            </div>
        </div>
    );
}
