'use client';

import { Globe } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { useTranslation } from '@/lib/i18n-context';
import { motion } from 'framer-motion';

interface LanguageToggleProps {
    className?: string;
    variant?: 'fixed' | 'inline';
}

export function LanguageToggle({ className, variant = 'fixed' }: LanguageToggleProps) {
    const { language, setLanguage } = useTranslation();

    const Content = (
        <DropdownMenu>
            <DropdownMenuTrigger asChild>
                <Button
                    variant="outline"
                    size="icon"
                    className={`rounded-full shadow-lg border-primary/20 bg-background/80 backdrop-blur-md hover:bg-primary/10 transition-all duration-300 group ${variant === 'fixed' ? 'h-12 w-12' : 'h-10 w-10 border-0 shadow-none bg-transparent'} ${className}`}
                >
                    <motion.div
                        whileHover={{ rotate: 180 }}
                        transition={{ duration: 0.5 }}
                    >
                        <Globe className={`${variant === 'fixed' ? 'h-6 w-6' : 'h-5 w-5'} text-primary`} />
                    </motion.div>
                </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="mb-2 min-w-[100px] rounded-xl p-2 animate-in slide-in-from-bottom-2 duration-300">
                <DropdownMenuItem
                    onClick={() => setLanguage('ru')}
                    className={`flex items-center justify-between rounded-lg px-3 py-2 cursor-pointer transition-colors ${language === 'ru' ? 'bg-primary/10 text-primary' : 'hover:bg-muted'}`}
                >
                    <span className="font-medium">RU</span>
                    {language === 'ru' && <div className="h-1.5 w-1.5 rounded-full bg-primary" />}
                </DropdownMenuItem>
                <DropdownMenuItem
                    onClick={() => setLanguage('en')}
                    className={`flex items-center justify-between rounded-lg px-3 py-2 cursor-pointer transition-colors ${language === 'en' ? 'bg-primary/10 text-primary' : 'hover:bg-muted'}`}
                >
                    <span className="font-medium">EN</span>
                    {language === 'en' && <div className="h-1.5 w-1.5 rounded-full bg-primary" />}
                </DropdownMenuItem>
            </DropdownMenuContent>
        </DropdownMenu>
    );

    if (variant === 'fixed') {
        return (
            <div className={`fixed bottom-6 right-6 z-50 ${className}`}>
                {Content}
            </div>
        );
    }

    return Content;
}
