/**
 * One entry point for every test download.
 *
 * Word, Excel and Markdown are rendered in the browser from the test payload the
 * page already holds; PDF is rendered by the backend, which owns the typography
 * the printed version needs. Each writer is imported lazily so pressing
 * "download" only pulls in the one format the user picked.
 */
import { Test } from '@/types/api';
import { downloadAllTestsPdf, downloadTestPdf } from '@/lib/api';
import { exportFileName } from './shared';

export type ExportFormat = 'pdf' | 'docx' | 'xlsx' | 'md';

export const EXPORT_FORMATS: ExportFormat[] = ['pdf', 'docx', 'xlsx', 'md'];

/** File extension per format — also what the format chips show in the UI. */
export const FORMAT_EXTENSION: Record<ExportFormat, string> = {
    pdf: 'PDF',
    docx: 'DOCX',
    xlsx: 'XLSX',
    md: 'MD',
};

/** Hands a locally built blob to the browser as a download. */
async function save(blob: Blob, fileName: string): Promise<string> {
    const { saveAs } = await import('file-saver');
    saveAs(blob, fileName);
    return fileName;
}

/** A download covers either one test or the whole library. */
export type ExportScope = 'test' | 'library';

/**
 * Downloads `tests` in the chosen format and resolves with the saved file name.
 *
 * A single test is exported on its own; a library is bundled into a catalogue
 * (cover sheet, contents, then one section per test). `tests` may be left empty
 * for a PDF of the library — the backend reads it server-side.
 */
export async function downloadTests(
    tests: Test[],
    lang: string,
    format: ExportFormat,
    scope: ExportScope = 'test',
): Promise<string> {
    if (format === 'pdf') {
        const fallback = exportFileName(tests, lang, 'pdf');
        return scope === 'library'
            ? downloadAllTestsPdf(lang, fallback)
            : downloadTestPdf(tests[0].id, lang, fallback);
    }

    if (tests.length === 0) {
        throw new Error('No tests to export');
    }

    switch (format) {
        case 'docx': {
            const { buildTestsDocx } = await import('./docx');
            const { blob, fileName } = await buildTestsDocx(tests, lang);
            return save(blob, fileName);
        }
        case 'xlsx': {
            const { buildTestsXlsx } = await import('./xlsx');
            const { blob, fileName } = await buildTestsXlsx(tests, lang);
            return save(blob, fileName);
        }
        case 'md': {
            const { buildTestsMarkdownFile } = await import('./markdown');
            const { blob, fileName } = buildTestsMarkdownFile(tests, lang);
            return save(blob, fileName);
        }
    }
}
