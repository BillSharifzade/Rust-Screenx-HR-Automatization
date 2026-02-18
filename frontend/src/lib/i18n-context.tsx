'use client';

import React, { createContext, useContext, useState, useEffect } from 'react';
import { ru } from './translations/ru';
import { en } from './translations/en';

type Language = 'ru' | 'en';
type Translations = typeof ru;

interface LanguageContextType {
    language: Language;
    setLanguage: (lang: Language) => void;
    t: (path: string) => string;
    translations: Translations;
}

const LanguageContext = createContext<LanguageContextType | undefined>(undefined);

export const LanguageProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    const [language, setLanguageState] = useState<Language>('ru');
    const [isLoaded, setIsLoaded] = useState(false);

    useEffect(() => {
        const savedLang = localStorage.getItem('language') as Language;
        if (savedLang && (savedLang === 'ru' || savedLang === 'en')) {
            setLanguageState(savedLang);
        }
        setIsLoaded(true);
    }, []);

    const setLanguage = (lang: Language) => {
        setLanguageState(lang);
        localStorage.setItem('language', lang);
        document.documentElement.lang = lang;
    };

    const translations = language === 'ru' ? ru : en;

    // Simple helper to get nested translations
    const t = (path: string): string => {
        const keys = path.split('.');
        let value: any = translations;

        for (const key of keys) {
            if (value && value[key] !== undefined) {
                value = value[key];
            } else {
                // If not found in primary path, try searching in 'common' if it's not already common
                if (keys[0] !== 'common') {
                    let commonValue: any = translations.common;
                    if (commonValue && commonValue[path] !== undefined) {
                        return commonValue[path];
                    }
                }

                console.warn(`Translation key not found: ${path}`);
                // Fallback to the last part of the key as a human-readable title-cased string
                const fallback = keys[keys.length - 1]
                    .replace(/_/g, ' ')
                    .replace(/\b\w/g, c => c.toUpperCase());
                return fallback;
            }
        }
        return value;
    };

    if (!isLoaded) {
        return null; // or a loader
    }

    return (
        <LanguageContext.Provider value={{ language, setLanguage, t, translations }}>
            {children}
        </LanguageContext.Provider>
    );
};

export const useTranslation = () => {
    const context = useContext(LanguageContext);
    if (context === undefined) {
        throw new Error('useTranslation must be used within a LanguageProvider');
    }
    return context;
};
