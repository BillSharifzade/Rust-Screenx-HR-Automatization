'use client';

import * as React from 'react';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

interface PaginationProps {
    page: number;
    totalPages: number;
    onPageChange: (page: number) => void;
    /** Optional caption rendered on the left, e.g. "1-10 of 42". */
    caption?: React.ReactNode;
    /** Disables every control while a page is being fetched. */
    disabled?: boolean;
    className?: string;
}

/**
 * Builds a compact page list around the current page:
 * 1 ... 4 5 6 ... 20  (ellipsis entries are represented by null)
 */
function buildPages(page: number, totalPages: number): (number | null)[] {
    if (totalPages <= 7) {
        return Array.from({ length: totalPages }, (_, i) => i + 1);
    }

    const pages: (number | null)[] = [1];
    const start = Math.max(2, page - 1);
    const end = Math.min(totalPages - 1, page + 1);

    if (start > 2) pages.push(null);
    for (let i = start; i <= end; i++) pages.push(i);
    if (end < totalPages - 1) pages.push(null);
    pages.push(totalPages);

    return pages;
}

export function Pagination({
    page,
    totalPages,
    onPageChange,
    caption,
    disabled = false,
    className,
}: PaginationProps) {
    if (totalPages <= 1) return null;

    const current = Math.min(Math.max(page, 1), totalPages);
    const go = (target: number) => {
        const next = Math.min(Math.max(target, 1), totalPages);
        if (next !== current) onPageChange(next);
    };

    return (
        <div
            className={cn(
                'flex flex-col-reverse sm:flex-row sm:items-center sm:justify-between gap-3 pt-2',
                className
            )}
        >
            <p className="text-sm text-muted-foreground">{caption}</p>

            <nav className="flex items-center gap-1" aria-label="Pagination">
                <Button
                    variant="outline"
                    size="icon-sm"
                    onClick={() => go(current - 1)}
                    disabled={disabled || current <= 1}
                    aria-label="Previous page"
                >
                    <ChevronLeft className="h-4 w-4" />
                </Button>

                {buildPages(current, totalPages).map((p, i) =>
                    p === null ? (
                        <span
                            key={`gap-${i}`}
                            className="px-2 text-sm text-muted-foreground select-none"
                        >
                            …
                        </span>
                    ) : (
                        <Button
                            key={p}
                            variant={p === current ? 'default' : 'outline'}
                            size="icon-sm"
                            onClick={() => go(p)}
                            disabled={disabled}
                            aria-current={p === current ? 'page' : undefined}
                            className="tabular-nums"
                        >
                            {p}
                        </Button>
                    )
                )}

                <Button
                    variant="outline"
                    size="icon-sm"
                    onClick={() => go(current + 1)}
                    disabled={disabled || current >= totalPages}
                    aria-label="Next page"
                >
                    <ChevronRight className="h-4 w-4" />
                </Button>
            </nav>
        </div>
    );
}

export default Pagination;
