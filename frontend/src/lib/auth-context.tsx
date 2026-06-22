'use client';

import React, { createContext, useContext, useEffect, useState, useCallback } from 'react';
import { useRouter } from 'next/navigation';
import { tokenStore } from './store';
import { AuthUser, fetchMe, logout as apiLogout } from './auth';

interface AuthContextType {
    user: AuthUser | null;
    loading: boolean;
    isAdmin: boolean;
    refresh: () => Promise<void>;
    logout: () => void;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: React.ReactNode }) {
    const [user, setUser] = useState<AuthUser | null>(null);
    const [loading, setLoading] = useState(true);
    const router = useRouter();

    const refresh = useCallback(async () => {
        if (!tokenStore.getToken()) {
            setUser(null);
            setLoading(false);
            return;
        }
        try {
            const me = await fetchMe();
            setUser(me);
        } catch {
            // Token invalid/expired -> drop it.
            tokenStore.removeToken();
            setUser(null);
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => {
        refresh();
    }, [refresh]);

    const logout = useCallback(() => {
        apiLogout();
        setUser(null);
        router.replace('/login');
    }, [router]);

    return (
        <AuthContext.Provider
            value={{ user, loading, isAdmin: user?.role === 'admin', refresh, logout }}
        >
            {children}
        </AuthContext.Provider>
    );
}

export function useAuth(): AuthContextType {
    const ctx = useContext(AuthContext);
    if (ctx === undefined) {
        throw new Error('useAuth must be used within an AuthProvider');
    }
    return ctx;
}
