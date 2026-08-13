/**
 * Builds the Excel export: one workbook with a sheet per kind of row rather than
 * a sheet per test, so the result stays filterable no matter how many tests are
 * exported.
 *
 *   Tests               — one row per test (the overview)
 *   Questions           — one row per question, with the options spread across columns
 *   Presentation topics — one row per topic (only when a presentation is included)
 *
 * `exceljs` is imported lazily: it is by far the heaviest of the writers.
 */
import type { Worksheet } from 'exceljs';
import { Test } from '@/types/api';
import {
    DocQuestion,
    Labels,
    OPTION_LETTERS,
    durationLabel,
    exportFileName,
    inline,
    isPresentation,
    labelsFor,
    questionTypeLabel,
    questionsOf,
    themesOf,
} from './shared';

const XLSX_MIME = 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet';

// ---- palette (shared with the Word/PDF exports) ----
const HEADER_BG = 'FF1D4ED8';
const HEADER_INK = 'FFFFFFFF';
const HAIRLINE = 'FFE5E7EB';
const CORRECT_INK = 'FF15803D';
const MUTED_INK = 'FF6B7280';

/** Applies the shared header treatment and locks the header row in place. */
function styleHeader(sheet: Worksheet): void {
    const header = sheet.getRow(1);
    header.font = { bold: true, size: 11, color: { argb: HEADER_INK } };
    header.alignment = { vertical: 'middle', horizontal: 'left', wrapText: true };
    header.height = 26;
    header.eachCell((cell) => {
        cell.fill = { type: 'pattern', pattern: 'solid', fgColor: { argb: HEADER_BG } };
    });
    sheet.views = [{ state: 'frozen', ySplit: 1 }];
    if (sheet.columnCount > 0) {
        sheet.autoFilter = {
            from: { row: 1, column: 1 },
            to: { row: 1, column: sheet.columnCount },
        };
    }
}

/** Hairline separators + top-aligned wrapped body text, applied to data rows. */
function styleBody(sheet: Worksheet): void {
    sheet.eachRow((row, rowNumber) => {
        if (rowNumber === 1) return;
        row.alignment = { vertical: 'top', wrapText: true };
        row.eachCell((cell) => {
            cell.border = {
                bottom: { style: 'hair', color: { argb: HAIRLINE } },
            };
        });
    });
}

/** Details that only apply to one question kind, collapsed into a single cell. */
function questionDetails(question: DocQuestion, labels: Labels): string {
    const parts: string[] = [];

    if (question.type === 'code') {
        parts.push(`${labels.language}: ${question.language || '—'}`);
        const starter = (question.starter_code ?? '').replace(/\r/g, '').trimEnd();
        if (starter.trim().length > 0) {
            parts.push(`${labels.starter_code}:\n${starter}`);
        }
    }

    if (question.type === 'short_answer') {
        if (question.min_words) parts.push(`${labels.min_words}: ${question.min_words}`);
        if (question.ai_grading) parts.push(labels.ai_grading);
        const keywords = (question.expected_keywords ?? []).map((k) => inline(k)).filter(Boolean);
        if (keywords.length > 0) parts.push(`${labels.keywords}: ${keywords.join(', ')}`);
    }

    return parts.join('\n');
}

function addOverviewSheet(
    workbook: InstanceType<typeof import('exceljs').Workbook>,
    tests: Test[],
    labels: Labels,
    lang: string,
): void {
    const sheet = workbook.addWorksheet(labels.sheet_overview);
    sheet.columns = [
        { header: labels.index_no, key: 'no', width: 6 },
        { header: labels.index_title, key: 'title', width: 42 },
        { header: labels.index_type, key: 'type', width: 16 },
        { header: labels.index_volume, key: 'volume', width: 10 },
        { header: labels.duration, key: 'duration', width: 12 },
        { header: labels.passing_score, key: 'score', width: 14 },
        { header: labels.attempts, key: 'attempts', width: 10 },
        { header: labels.status, key: 'status', width: 12 },
        { header: labels.created, key: 'created', width: 14 },
        { header: labels.description, key: 'description', width: 60 },
    ];

    tests.forEach((test, idx) => {
        const presentation = isPresentation(test);
        sheet.addRow({
            no: idx + 1,
            title: inline(test.title),
            type: presentation ? labels.type_presentation : labels.type_test,
            volume: presentation ? themesOf(test).length : questionsOf(test).length,
            duration: durationLabel(test, labels),
            // passing_score arrives as a decimal string, so "70.00" becomes 70%.
            score: (Number(test.passing_score) || 0) / 100,
            attempts: test.max_attempts || 1,
            status: test.is_active === false ? labels.archived : labels.active,
            created: test.created_at
                ? new Date(test.created_at).toLocaleDateString(lang === 'en' ? 'en-GB' : 'ru-RU')
                : '—',
            description: inline(test.description) || labels.no_description,
        });
    });

    sheet.getColumn('score').numFmt = '0%';
    sheet.getColumn('no').alignment = { horizontal: 'center', vertical: 'top' };
    styleHeader(sheet);
    styleBody(sheet);
}

function addQuestionsSheet(
    workbook: InstanceType<typeof import('exceljs').Workbook>,
    tests: Test[],
    labels: Labels,
    withTestColumn: boolean,
): void {
    const questionTests = tests.filter((test) => !isPresentation(test));
    if (questionTests.length === 0) return;

    const maxOptions = Math.min(
        questionTests.reduce(
            (max, test) =>
                questionsOf(test).reduce(
                    (inner, question) => Math.max(inner, question.options?.length ?? 0),
                    max,
                ),
            0,
        ),
        OPTION_LETTERS.length,
    );

    const sheet = workbook.addWorksheet(labels.sheet_questions);
    sheet.columns = [
        ...(withTestColumn ? [{ header: labels.test, key: 'test', width: 30 }] : []),
        { header: labels.number, key: 'no', width: 6 },
        { header: labels.question, key: 'question', width: 60 },
        { header: labels.type, key: 'type', width: 16 },
        { header: labels.points_column, key: 'points', width: 8 },
        ...Array.from({ length: maxOptions }, (_, idx) => ({
            header: OPTION_LETTERS[idx],
            key: `option_${idx}`,
            width: 26,
        })),
        { header: labels.correct, key: 'correct', width: 16 },
        { header: labels.details, key: 'details', width: 40 },
        { header: labels.explanation, key: 'explanation', width: 40 },
    ];

    questionTests.forEach((test) => {
        const questions = questionsOf(test);
        if (questions.length === 0) {
            const empty = sheet.addRow({
                ...(withTestColumn ? { test: inline(test.title) } : {}),
                question: labels.no_questions,
            });
            empty.getCell('question').font = { italic: true, color: { argb: MUTED_INK } };
            return;
        }

        questions.forEach((question, idx) => {
            const options = Array.isArray(question.options) ? question.options : [];
            const optionCells: Record<string, string> = {};
            options.slice(0, maxOptions).forEach((option, optIdx) => {
                optionCells[`option_${optIdx}`] = inline(option);
            });

            const correctIndex = question.correct_answer;
            const correct =
                question.type === 'multiple_choice' &&
                typeof correctIndex === 'number' &&
                correctIndex >= 0 &&
                correctIndex < options.length
                    ? `${OPTION_LETTERS[correctIndex] ?? correctIndex + 1}) ${inline(options[correctIndex])}`
                    : '';

            const row = sheet.addRow({
                ...(withTestColumn ? { test: inline(test.title) } : {}),
                no: idx + 1,
                question: inline(question.question) || '—',
                type: questionTypeLabel(question, labels),
                points: question.points ?? 1,
                ...optionCells,
                correct,
                details: questionDetails(question, labels),
                explanation: inline(question.explanation),
            });

            row.getCell('correct').font = { bold: true, color: { argb: CORRECT_INK } };
            if (typeof correctIndex === 'number' && correctIndex < maxOptions) {
                row.getCell(`option_${correctIndex}`).font = { bold: true, color: { argb: CORRECT_INK } };
            }
            if (question.type === 'code') {
                row.getCell('details').font = { name: 'Consolas', size: 10 };
            }
        });
    });

    sheet.getColumn('no').alignment = { horizontal: 'center', vertical: 'top' };
    sheet.getColumn('points').alignment = { horizontal: 'center', vertical: 'top' };
    styleHeader(sheet);
    styleBody(sheet);
}

function addThemesSheet(
    workbook: InstanceType<typeof import('exceljs').Workbook>,
    tests: Test[],
    labels: Labels,
    withTestColumn: boolean,
): void {
    const presentations = tests.filter(isPresentation);
    if (presentations.length === 0) return;

    const sheet = workbook.addWorksheet(labels.sheet_themes);
    sheet.columns = [
        ...(withTestColumn ? [{ header: labels.test, key: 'test', width: 30 }] : []),
        { header: labels.number, key: 'no', width: 6 },
        { header: labels.theme, key: 'theme', width: 60 },
        { header: labels.extra_info, key: 'extra', width: 50 },
    ];

    presentations.forEach((test) => {
        const extra = inline(test.presentation_extra_info);
        const themes = themesOf(test);
        if (themes.length === 0) {
            sheet.addRow({
                ...(withTestColumn ? { test: inline(test.title) } : {}),
                extra,
            });
            return;
        }
        themes.forEach((theme, idx) => {
            sheet.addRow({
                ...(withTestColumn ? { test: inline(test.title) } : {}),
                no: idx + 1,
                theme: inline(theme),
                // The extra info belongs to the test, so it rides on its first row only.
                extra: idx === 0 ? extra : '',
            });
        });
    });

    sheet.getColumn('no').alignment = { horizontal: 'center', vertical: 'top' };
    styleHeader(sheet);
    styleBody(sheet);
}

/** Renders the given tests into one .xlsx, plus the file name to save it under. */
export async function buildTestsXlsx(
    tests: Test[],
    lang: string,
): Promise<{ blob: Blob; fileName: string }> {
    const excel = await import('exceljs');
    // The browser build is UMD: depending on the bundler the classes hang off
    // the module namespace or off its default export.
    const ExcelJS = (excel as unknown as { default?: typeof excel }).default ?? excel;

    const labels = labelsFor(lang);
    const single = tests.length === 1;

    const workbook = new ExcelJS.Workbook();
    workbook.creator = labels.brand;
    workbook.created = new Date();
    workbook.title = single ? inline(tests[0].title) : labels.cover_title;

    // The overview carries the meta (duration, passing score, attempts) that the
    // detail sheets have no column for, so it leads even for a single test.
    addOverviewSheet(workbook, tests, labels, lang);
    addQuestionsSheet(workbook, tests, labels, !single);
    addThemesSheet(workbook, tests, labels, !single);

    const buffer = await workbook.xlsx.writeBuffer();
    const blob = new Blob([buffer as ArrayBuffer], { type: XLSX_MIME });
    return { blob, fileName: exportFileName(tests, lang, 'xlsx') };
}
