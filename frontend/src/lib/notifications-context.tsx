'use client';

import React, { createContext, useContext, useEffect, useState, useRef } from 'react';
import { toast } from 'sonner';
import { apiFetch } from '@/lib/api';
import { usePathname } from 'next/navigation';
import { useTranslation } from './i18n-context';
import { useQueryClient } from '@tanstack/react-query';

interface NotificationCounts {
    candidates: number;
    attempts: number;
}

interface NotificationsContextType {
    counts: NotificationCounts;
    resetCount: (type: keyof NotificationCounts) => void;
}

const NotificationsContext = createContext<NotificationsContextType>({
    counts: { candidates: 0, attempts: 0 },
    resetCount: () => { },
});

export const useNotifications = () => useContext(NotificationsContext);

export function NotificationsProvider({ children }: { children: React.ReactNode }) {
    const { t } = useTranslation();
    const [counts, setCounts] = useState<NotificationCounts>({ candidates: 0, attempts: 0 });
    const lastCheckRef = useRef<string>(new Date().toISOString());
    const pathname = usePathname();

    // Reset counts when visiting relevant pages
    useEffect(() => {
        if (pathname === '/dashboard/candidates') {
            setCounts(prev => ({ ...prev, candidates: 0 }));
        } else if (pathname === '/dashboard/attempts' || pathname === '/dashboard/invites') {
            setCounts(prev => ({ ...prev, attempts: 0 }));
        }
    }, [pathname]);

    const queryClient = useQueryClient();

    useEffect(() => {
        const poll = async () => {
            try {
                const now = new Date().toISOString();
                const since = lastCheckRef.current;

                // Fetch notifications
                const data: any = await apiFetch(`/api/integration/notifications/poll?since=${since}`);

                // Update timestamp for next poll
                lastCheckRef.current = now;

                let newCandidates = 0;
                let newAttempts = 0;

                if (data.candidates && data.candidates.length > 0) {
                    newCandidates = data.candidates.length;

                    // Always refresh data if we're on the page
                    if (pathname === '/dashboard/candidates') {
                        queryClient.invalidateQueries({ queryKey: ["candidates"] });
                    }

                    // Show toast only if not on the page
                    if (pathname !== '/dashboard/candidates') {
                        toast.info(t('notifications.new_candidates').replace('{count}', newCandidates.toString()), {
                            description: t('notifications.last_candidate').replace('{name}', data.candidates[0].name)
                        });
                    }
                }

                if (data.attempts && data.attempts.length > 0) {
                    newAttempts = data.attempts.length;

                    // Refresh data if on relevant pages
                    if (pathname === '/dashboard/attempts' || pathname === '/dashboard/invites') {
                        queryClient.invalidateQueries({ queryKey: ["test-attempts"] });
                        queryClient.invalidateQueries({ queryKey: ["invites"] });
                    }

                    // Filter vital updates?
                    const completed = data.attempts.filter((a: any) => a.status === 'completed' || a.status === 'needs_review');

                    if (pathname !== '/dashboard/attempts' && completed.length > 0) {
                        toast.success(t('notifications.test_updates').replace('{count}', completed.length.toString()), {
                            description: t('notifications.check_attempts')
                        });
                    }
                }

                if (data.counts) {
                    setCounts({
                        candidates: pathname === '/dashboard/candidates' ? 0 : data.counts.candidates,
                        attempts: (pathname === '/dashboard/attempts' || pathname === '/dashboard/invites') ? 0 : data.counts.attempts,
                    });
                }

            } catch (e) {
                console.error("Poll error:", e);
            }
        };

        const intervalId = setInterval(poll, 15000); // 15 seconds
        return () => clearInterval(intervalId);
    }, [pathname, queryClient]);

    const resetCount = (type: keyof NotificationCounts) => {
        setCounts(prev => ({ ...prev, [type]: 0 }));
    };

    return (
        <NotificationsContext.Provider value={{ counts, resetCount }}>
            {children}
        </NotificationsContext.Provider>
    );
}
