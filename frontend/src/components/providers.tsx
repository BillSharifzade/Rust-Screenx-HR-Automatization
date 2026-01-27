'use client';

import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ThemeProvider } from '@/components/theme-provider';
import { Toaster } from 'sonner';
import { useState } from 'react';

import { LanguageProvider } from '@/lib/i18n-context';

export function Providers({ children }: { children: React.ReactNode }) {
    const [queryClient] = useState(() => new QueryClient());

    return (
        <QueryClientProvider client={queryClient}>
            <ThemeProvider
                attribute="class"
                defaultTheme="system"
                enableSystem
                disableTransitionOnChange
            >
                <LanguageProvider>
                    {children}
                    <Toaster position="top-center" />
                </LanguageProvider>
            </ThemeProvider>
        </QueryClientProvider>
    );
}
