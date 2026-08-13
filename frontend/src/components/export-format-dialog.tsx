'use client';

/**
 * Format picker shown before a test download starts: PDF, Word, Excel or
 * Markdown. It only chooses — the caller runs the export and keeps the dialog
 * open (with the chosen tile spinning) until the file is on disk.
 */
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
} from '@/components/ui/dialog';
import { Loader2 } from 'lucide-react';
import { useTranslation } from '@/lib/i18n-context';
import { EXPORT_FORMATS, ExportFormat, FORMAT_EXTENSION } from '@/lib/tests-export';
import { cn } from '@/lib/utils';

/** A page with a folded corner and the extension on its ribbon. */
function FormatGlyph({ label, className }: { label: string; className?: string }) {
    return (
        <svg
            viewBox="0 0 40 48"
            className={cn('h-10 w-9 shrink-0', className)}
            aria-hidden="true"
            focusable="false"
        >
            <path
                d="M5 5a4 4 0 0 1 4-4h15l13 13v29a4 4 0 0 1-4 4H9a4 4 0 0 1-4-4Z"
                fill="currentColor"
                opacity="0.12"
            />
            <path
                d="M5 5a4 4 0 0 1 4-4h15l13 13v29a4 4 0 0 1-4 4H9a4 4 0 0 1-4-4Z"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                opacity="0.55"
            />
            <path
                d="M24 1v8a4 4 0 0 0 4 4h9"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                opacity="0.55"
            />
            <rect x="0" y="24" width="33" height="15" rx="3.5" fill="currentColor" />
            <text
                x="16.5"
                y="34.6"
                textAnchor="middle"
                fill="#fff"
                fontSize="9.5"
                fontWeight="700"
                letterSpacing="0.3"
                fontFamily="inherit"
            >
                {label}
            </text>
        </svg>
    );
}

/** Each format keeps the colour its app is known by. */
const FORMAT_TINT: Record<ExportFormat, string> = {
    pdf: 'text-red-600 dark:text-red-400',
    docx: 'text-blue-600 dark:text-blue-400',
    xlsx: 'text-emerald-600 dark:text-emerald-400',
    md: 'text-violet-600 dark:text-violet-400',
};

const FORMAT_NAME: Record<ExportFormat, string> = {
    pdf: 'PDF',
    docx: 'Word',
    xlsx: 'Excel',
    md: 'Markdown',
};

interface ExportFormatDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    title: string;
    description: string;
    /** The format currently being written, if any — its tile shows a spinner. */
    busyFormat?: ExportFormat | null;
    onSelect: (format: ExportFormat) => void;
}

export function ExportFormatDialog({
    open,
    onOpenChange,
    title,
    description,
    busyFormat = null,
    onSelect,
}: ExportFormatDialogProps) {
    const { t } = useTranslation();
    const busy = busyFormat !== null;

    return (
        <Dialog
            open={open}
            onOpenChange={(next) => {
                // Closing mid-write would leave a download with no feedback.
                if (busy && !next) return;
                onOpenChange(next);
            }}
        >
            <DialogContent className="sm:max-w-lg">
                <DialogHeader>
                    <DialogTitle>{title}</DialogTitle>
                    <DialogDescription>{description}</DialogDescription>
                </DialogHeader>

                <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                    {EXPORT_FORMATS.map((format) => {
                        const isBusy = busyFormat === format;
                        return (
                            <button
                                key={format}
                                type="button"
                                disabled={busy}
                                onClick={() => onSelect(format)}
                                className={cn(
                                    'group flex items-center gap-3 rounded-lg border bg-card p-4 text-left',
                                    'transition-colors hover:border-primary hover:bg-accent/40',
                                    'focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-none',
                                    'disabled:cursor-not-allowed disabled:opacity-60',
                                    isBusy && 'border-primary bg-accent/40',
                                )}
                            >
                                <FormatGlyph
                                    label={FORMAT_EXTENSION[format]}
                                    className={FORMAT_TINT[format]}
                                />
                                <span className="min-w-0 flex-1">
                                    <span className="block text-sm font-semibold">
                                        {FORMAT_NAME[format]}
                                    </span>
                                    <span className="text-muted-foreground block text-xs leading-snug">
                                        {t(`dashboard.tests.export.${format}_desc`)}
                                    </span>
                                </span>
                                {isBusy && (
                                    <Loader2 className="text-primary h-4 w-4 shrink-0 animate-spin" />
                                )}
                            </button>
                        );
                    })}
                </div>
            </DialogContent>
        </Dialog>
    );
}
