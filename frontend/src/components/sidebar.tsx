'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { useState } from 'react';
import {
  Home,
  Users,
  FileText,
  Briefcase,
  ClipboardCheck,
  User,
  ChevronLeft,
  ChevronRight
} from 'lucide-react';
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { ModeToggle } from "@/components/mode-toggle";
import { motion, AnimatePresence } from "framer-motion";

import { useTranslation } from "@/lib/i18n-context";
import { LanguageToggle } from "@/components/language-toggle";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useNotifications } from '@/lib/notifications-context';

const AnimatedHamburger = ({ isOpen }: { isOpen: boolean }) => {
  return (
    <div className="relative flex flex-col items-center justify-center w-5 h-5 gap-1">
      <motion.span
        animate={isOpen ? { rotate: 45, y: 6 } : { rotate: 0, y: 0 }}
        transition={{ type: "spring", stiffness: 300, damping: 20 }}
        className="w-5 h-0.5 bg-foreground rounded-full"
      />
      <motion.span
        animate={isOpen ? { opacity: 0, x: -10 } : { opacity: 1, x: 0 }}
        transition={{ duration: 0.2 }}
        className="w-5 h-0.5 bg-foreground rounded-full"
      />
      <motion.span
        animate={isOpen ? { rotate: -45, y: -6 } : { rotate: 0, y: 0 }}
        transition={{ type: "spring", stiffness: 300, damping: 20 }}
        className="w-5 h-0.5 bg-foreground rounded-full"
      />
    </div>
  );
};

export function Sidebar() {
  const { t } = useTranslation();
  const { counts } = useNotifications();
  const pathname = usePathname();
  const [isCollapsed, setIsCollapsed] = useState(false);
  const [isMenuOpen, setIsMenuOpen] = useState(false);

  const navItems = [
    { name: t('dashboard.nav.dashboard'), href: '/dashboard', icon: Home },
    { name: t('dashboard.nav.vacancies'), href: '/dashboard/vacancies', icon: Briefcase },
    { name: t('dashboard.nav.tests'), href: '/dashboard/tests', icon: FileText },
    { name: t('dashboard.nav.invites'), href: '/dashboard/invites', icon: Users },
    { name: t('dashboard.nav.candidates'), href: '/dashboard/candidates', icon: User },
    { name: t('dashboard.nav.attempts'), href: '/dashboard/attempts', icon: ClipboardCheck },
  ];

  return (
    <motion.div
      initial={{ width: 240 }}
      animate={{ width: isCollapsed ? 80 : 240 }}
      transition={{ duration: 0.3, type: "spring", stiffness: 100, damping: 20 }}
      className="hidden border-r bg-muted/40 md:flex flex-col relative z-20 h-screen sticky top-0"
    >
      <div className={cn(
        "flex h-16 items-center border-b bg-background relative transition-all",
        isCollapsed ? "justify-center px-2" : "justify-between px-4"
      )}>
        <Link href="/" className="flex items-center gap-2 font-semibold group shrink-0">
          <div className="h-8 w-8 min-w-[32px] rounded-lg bg-primary flex items-center justify-center text-primary-foreground font-bold text-lg shrink-0 shadow-sm shadow-primary/20">
            K
          </div>
          <AnimatePresence mode="wait">
            {!isCollapsed && (
              <motion.span
                initial={{ opacity: 0, width: 0 }}
                animate={{ opacity: 1, width: "auto" }}
                exit={{ opacity: 0, width: 0 }}
                transition={{ duration: 0.2 }}
                className="text-xl font-bold bg-gradient-to-r from-foreground to-foreground/70 bg-clip-text whitespace-nowrap overflow-hidden"
              >
                KoinotHR
              </motion.span>
            )}
          </AnimatePresence>
        </Link>
        <div className="flex items-center gap-1">
          {!isCollapsed && (
            <DropdownMenu onOpenChange={setIsMenuOpen}>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="icon" className="h-8 w-8 rounded-full hover:bg-muted/50 transition-colors">
                  <AnimatedHamburger isOpen={isMenuOpen} />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start" side="right" className="min-w-fit ml-2 rounded-xl border-primary/10 shadow-xl p-1 animate-in slide-in-from-left-2 duration-200">
                <div className="flex items-center gap-1">
                  <LanguageToggle variant="inline" />
                  <ModeToggle />
                </div>
              </DropdownMenuContent>
            </DropdownMenu>
          )}
          <Button
            variant="ghost"
            size="icon"
            className={cn(
              "h-8 w-8 rounded-full text-muted-foreground hover:text-primary transition-all",
              isCollapsed && "absolute -right-3 top-4 bg-background border shadow-md z-30 h-6 w-6"
            )}
            onClick={() => setIsCollapsed(!isCollapsed)}
          >
            {isCollapsed ? <ChevronRight className="h-4 w-4" /> : <ChevronLeft className="h-4 w-4" />}
          </Button>
        </div>
      </div>

      <div className="flex-1 py-4 px-3 flex flex-col gap-1 overflow-y-auto">
        <nav className="grid gap-1">
          {navItems.map((item) => {
            const isActive = item.href === '/dashboard'
              ? pathname === '/dashboard'
              : pathname === item.href || pathname.startsWith(item.href + '/');

            const isDashboard = item.href === '/dashboard';

            return (
              <div key={item.href} className="relative group/item">
                <Link
                  href={item.href}
                  className={cn(
                    "flex items-center rounded-lg px-3 py-2.5 transition-all relative group h-10 overflow-hidden",
                    isCollapsed ? "justify-center" : "gap-3",
                    isActive
                      ? 'bg-primary text-primary-foreground shadow-sm'
                      : 'text-muted-foreground hover:text-primary hover:bg-muted'
                  )}
                  title={isCollapsed ? item.name : undefined}
                >
                  <item.icon className="h-5 w-5 shrink-0" />
                  <AnimatePresence mode="wait">
                    {!isCollapsed && (
                      <motion.span
                        initial={{ opacity: 0, width: 0 }}
                        animate={{ opacity: 1, width: "auto" }}
                        exit={{ opacity: 0, width: 0 }}
                        transition={{ duration: 0.2 }}
                        className="font-medium whitespace-nowrap overflow-hidden text-sm flex-1"
                      >
                        {item.name}
                      </motion.span>
                    )}
                  </AnimatePresence>

                  {/* Badges */}
                  {item.href === '/dashboard/candidates' && counts.candidates > 0 && (
                    isCollapsed ? (
                      <span className="absolute top-1 right-1 w-2.5 h-2.5 bg-destructive rounded-full border-2 border-background" />
                    ) : (
                      <motion.span
                        initial={{ scale: 0 }}
                        animate={{ scale: 1 }}
                        className="bg-destructive text-destructive-foreground text-[10px] h-5 min-w-[20px] px-1 flex items-center justify-center rounded-full font-bold"
                      >
                        {counts.candidates}
                      </motion.span>
                    )
                  )}
                  {item.href === '/dashboard/attempts' && counts.attempts > 0 && (
                    isCollapsed ? (
                      <span className="absolute top-1 right-1 w-2.5 h-2.5 bg-destructive rounded-full border-2 border-background" />
                    ) : (
                      <motion.span
                        initial={{ scale: 0 }}
                        animate={{ scale: 1 }}
                        className="bg-destructive text-destructive-foreground text-[10px] h-5 min-w-[20px] px-1 flex items-center justify-center rounded-full font-bold"
                      >
                        {counts.attempts}
                      </motion.span>
                    )
                  )}

                  {isActive && isCollapsed && (
                    <div className="absolute left-0 top-0 bottom-0 w-1 bg-primary rounded-r-full" />
                  )}
                </Link>

              </div>
            );
          })}
        </nav>
      </div>

      {/* Subtle credit */}
      <AnimatePresence mode="wait">
        {!isCollapsed && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="px-4 py-3 border-t"
          >
            <a
              href="https://billsharifzade.github.io/"
              target="_blank"
              rel="noopener noreferrer"
              className="block text-[10px] text-muted-foreground/40 text-center select-none hover:text-primary/60 transition-all duration-300 hover:drop-shadow-[0_0_8px_rgba(var(--primary),0.3)]"
            >
              Crafted by qwantum
            </a>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
}
