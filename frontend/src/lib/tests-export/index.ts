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

/**
 * Downloads `tests` in the chosen format and resolves with the saved file name.
 *
 * A single test is exported on its own; two or more are bundled into a
 * catalogue (cover sheet, contents, then one section per test).
 */
export async function downloadTests(
    tests: Test[],
    lang: string,
    format: ExportFormat,
): Promise<string> {
    if (tests.length === 0) {
        throw new Error('No tests to export');
    }

    switch (format) {
        case 'pdf': {
            const fallback = exportFileName(tests, lang, 'pdf');
            return tests.length === 1
                ? downloadTestPdf(tests[0].id, lang, fallback)
                : downloadAllTestsPdf(lang, fallback);
        }
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
