/**
 * Builds a print-ready Word document from one test or from the whole library.
 *
 * The layout mirrors the PDF (see backend `pdf_service.rs`) so both exports read
 * the same, but is rebuilt with Word primitives. A catalogue opens with a cover
 * sheet and a contents table, then gives every test its own page; a single test
 * skips straight to its own section.
 *
 * `docx` and `file-saver` are imported lazily so the ~500 KB writer only lands
 * in the bundle of users who actually press the button.
 */
import type { IParagraphOptions, Paragraph as ParagraphNode, Table as TableNode } from 'docx';
import { Test } from '@/types/api';
import {
    DocQuestion,
    Labels,
    OPTION_LETTERS,
    block,
    durationLabel,
    exportFileName,
    formatDate,
    inline,
    isPresentation,
    labelsFor,
    questionsOf,
    themesOf,
    totalDurationLabel,
} from './shared';

type Node = ParagraphNode | TableNode;

// ---- palette (shared with the PDF exporter) ----
const INK = '111827';
const MUTED = '6B7280';
const ACCENT = '1D4ED8';
const CORRECT = '15803D';
const HAIRLINE = 'E5E7EB';
const SURFACE = 'F7F8FA';
const SURFACE_ACCENT = 'EFF4FF';

const BODY_FONT = 'Calibri';
const MONO_FONT = 'Consolas';

/**
 * Renders the given tests into one .docx, plus the file name it should be saved
 * under. A single test is rendered on its own; two or more get the catalogue
 * treatment (cover sheet + contents table).
 */
export async function buildTestsDocx(
    tests: Test[],
    lang: string,
): Promise<{ blob: Blob; fileName: string }> {
    const single = tests.length === 1;
    const docx = await import('docx');
    const {
        AlignmentType,
        BorderStyle,
        Document,
        Footer,
        Header,
        HeadingLevel,
        PageBreak,
        PageNumber,
        Packer,
        Paragraph,
        ShadingType,
        Table,
        TableCell,
        TableLayoutType,
        TableRow,
        TextRun,
        VerticalAlign,
        WidthType,
        convertMillimetersToTwip,
    } = docx;

    const labels: Labels = labelsFor(lang);
    const now = new Date();

    // A4 minus the left/right margins declared in `pageSetup`.
    const CONTENT_WIDTH = convertMillimetersToTwip(170);
    /** Splits the text column into exact twip widths so Word can't re-flow them. */
    const columns = (ratios: number[]) => {
        const total = ratios.reduce((sum, ratio) => sum + ratio, 0);
        return ratios.map((ratio) => Math.round((CONTENT_WIDTH * ratio) / total));
    };

    const hairline = { style: BorderStyle.SINGLE, size: 4, color: HAIRLINE } as const;
    const noBorder = { style: BorderStyle.NONE, size: 0, color: 'FFFFFF' } as const;
    const noBorders = {
        top: noBorder,
        bottom: noBorder,
        left: noBorder,
        right: noBorder,
        insideHorizontal: noBorder,
        insideVertical: noBorder,
    };

    // ---- small paragraph builders ----

    /** Uppercase, letter-spaced micro-label — the recurring accent of the layout. */
    const eyebrow = (text: string, color = MUTED, spacingAfter = 60) =>
        new Paragraph({
            spacing: { after: spacingAfter },
            children: [
                new TextRun({
                    text: text.toUpperCase(),
                    size: 15,
                    bold: true,
                    color,
                    characterSpacing: 30,
                    font: BODY_FONT,
                }),
            ],
        });

    const body = (
        text: string,
        opts: { color?: string; size?: number; italics?: boolean; indent?: number; after?: number } = {},
    ) =>
        new Paragraph({
            spacing: { after: opts.after ?? 80, line: 300 },
            indent: opts.indent ? { left: opts.indent } : undefined,
            children: [
                new TextRun({
                    text,
                    size: opts.size ?? 21,
                    color: opts.color ?? INK,
                    italics: opts.italics,
                    font: BODY_FONT,
                }),
            ],
        });

    /** A full-width rule; `thick` marks the accent rule used on the cover. */
    const rule = (color = HAIRLINE, thick = false, spacing = 120) =>
        new Paragraph({
            spacing: { before: spacing, after: spacing },
            border: { bottom: { style: BorderStyle.SINGLE, size: thick ? 18 : 6, color } },
            children: [new TextRun({ text: '', size: 2 })],
        });

    const spacer = (height: number) =>
        new Paragraph({ spacing: { after: height }, children: [new TextRun({ text: '', size: 2 })] });

    /** Label-over-value cell used by the cover stats and the per-test meta strip. */
    const statCell = (label: string, value: string, width: number) =>
        new TableCell({
            width: { size: width, type: WidthType.DXA },
            shading: { type: ShadingType.CLEAR, fill: SURFACE, color: 'auto' },
            margins: { top: 140, bottom: 140, left: 160, right: 160 },
            verticalAlign: VerticalAlign.CENTER,
            children: [
                new Paragraph({
                    spacing: { after: 40 },
                    children: [
                        new TextRun({
                            text: label.toUpperCase(),
                            size: 14,
                            bold: true,
                            color: MUTED,
                            characterSpacing: 24,
                            font: BODY_FONT,
                        }),
                    ],
                }),
                new Paragraph({
                    children: [
                        new TextRun({ text: value, size: 22, bold: true, color: INK, font: BODY_FONT }),
                    ],
                }),
            ],
        });

    /** Stat strip: hairline top/bottom, cells split by a white gutter. */
    const statStrip = (cells: { label: string; value: string }[]) => {
        const widths = columns(cells.map(() => 1));
        return new Table({
            width: { size: CONTENT_WIDTH, type: WidthType.DXA },
            columnWidths: widths,
            layout: TableLayoutType.FIXED,
            borders: {
                top: hairline,
                bottom: hairline,
                left: noBorder,
                right: noBorder,
                insideHorizontal: noBorder,
                insideVertical: { style: BorderStyle.SINGLE, size: 4, color: 'FFFFFF' },
            },
            rows: [
                new TableRow({
                    cantSplit: true,
                    children: cells.map((cell, idx) => statCell(cell.label, cell.value, widths[idx])),
                }),
            ],
        });
    };

    /** Tinted box with an accent bar on the left — used for instructions / extra info. */
    const calloutBox = (title: string, lines: string[]) =>
        new Table({
            width: { size: CONTENT_WIDTH, type: WidthType.DXA },
            columnWidths: [CONTENT_WIDTH],
            layout: TableLayoutType.FIXED,
            borders: {
                ...noBorders,
                left: { style: BorderStyle.SINGLE, size: 18, color: ACCENT },
            },
            rows: [
                new TableRow({
                    children: [
                        new TableCell({
                            width: { size: CONTENT_WIDTH, type: WidthType.DXA },
                            shading: { type: ShadingType.CLEAR, fill: SURFACE_ACCENT, color: 'auto' },
                            margins: { top: 160, bottom: 160, left: 220, right: 220 },
                            children: [
                                eyebrow(title, ACCENT, 80),
                                ...lines.map((line, idx) =>
                                    body(line, {
                                        size: 20,
                                        color: INK,
                                        after: idx === lines.length - 1 ? 0 : 80,
                                    }),
                                ),
                            ],
                        }),
                    ],
                }),
            ],
        });

    // ---- cover ----

    const totalQuestions = tests.reduce((sum, test) => sum + questionsOf(test).length, 0);
    const totalMinutes = tests.reduce((sum, test) => sum + (test.duration_minutes || 0), 0);

    const cover: Node[] = [
        spacer(1600),
        rule(ACCENT, true, 0),
        eyebrow(labels.brand, ACCENT, 240),
        new Paragraph({
            spacing: { after: 140 },
            children: [
                new TextRun({ text: labels.cover_title, size: 76, bold: true, color: INK, font: BODY_FONT }),
            ],
        }),
        new Paragraph({
            spacing: { after: 120 },
            children: [
                new TextRun({ text: labels.cover_subtitle, size: 26, color: MUTED, font: BODY_FONT }),
            ],
        }),
        new Paragraph({
            spacing: { after: 700 },
            children: [
                new TextRun({
                    text: `${labels.generated}: ${formatDate(now, lang)}`,
                    size: 20,
                    color: MUTED,
                    font: BODY_FONT,
                }),
            ],
        }),
        statStrip([
            { label: labels.total_tests, value: String(tests.length) },
            { label: labels.total_questions, value: String(totalQuestions) },
            { label: labels.total_duration, value: totalDurationLabel(totalMinutes, labels) },
        ]),
    ];

    // ---- contents ----

    const INDEX_COLUMNS = columns([7, 45, 18, 14, 16]);

    const indexHeaderCell = (
        text: string,
        width: number,
        align: (typeof AlignmentType)[keyof typeof AlignmentType],
    ) =>
        new TableCell({
            width: { size: width, type: WidthType.DXA },
            shading: { type: ShadingType.CLEAR, fill: SURFACE, color: 'auto' },
            margins: { top: 120, bottom: 120, left: 140, right: 140 },
            children: [
                new Paragraph({
                    alignment: align,
                    children: [
                        new TextRun({
                            text: text.toUpperCase(),
                            size: 15,
                            bold: true,
                            color: MUTED,
                            characterSpacing: 24,
                            font: BODY_FONT,
                        }),
                    ],
                }),
            ],
        });

    const indexCell = (
        text: string,
        width: number,
        align: (typeof AlignmentType)[keyof typeof AlignmentType],
        opts: { bold?: boolean; color?: string } = {},
    ) =>
        new TableCell({
            width: { size: width, type: WidthType.DXA },
            margins: { top: 120, bottom: 120, left: 140, right: 140 },
            verticalAlign: VerticalAlign.CENTER,
            children: [
                new Paragraph({
                    alignment: align,
                    children: [
                        new TextRun({
                            text,
                            size: 20,
                            bold: opts.bold,
                            color: opts.color ?? INK,
                            font: BODY_FONT,
                        }),
                    ],
                }),
            ],
        });

    const contents: Node[] = [
        eyebrow(labels.brand, ACCENT, 60),
        new Paragraph({
            spacing: { after: 60 },
            children: [
                new TextRun({ text: labels.contents, size: 44, bold: true, color: INK, font: BODY_FONT }),
            ],
        }),
        rule(HAIRLINE, false, 100),
        new Table({
            width: { size: CONTENT_WIDTH, type: WidthType.DXA },
            columnWidths: INDEX_COLUMNS,
            layout: TableLayoutType.FIXED,
            borders: {
                top: noBorder,
                bottom: hairline,
                left: noBorder,
                right: noBorder,
                insideHorizontal: hairline,
                insideVertical: noBorder,
            },
            rows: [
                new TableRow({
                    tableHeader: true,
                    children: [
                        indexHeaderCell(labels.index_no, INDEX_COLUMNS[0], AlignmentType.LEFT),
                        indexHeaderCell(labels.index_title, INDEX_COLUMNS[1], AlignmentType.LEFT),
                        indexHeaderCell(labels.index_type, INDEX_COLUMNS[2], AlignmentType.LEFT),
                        indexHeaderCell(labels.index_volume, INDEX_COLUMNS[3], AlignmentType.RIGHT),
                        indexHeaderCell(labels.index_duration, INDEX_COLUMNS[4], AlignmentType.RIGHT),
                    ],
                }),
                ...tests.map((test, idx) => {
                    const presentation = isPresentation(test);
                    const volume = String(
                        presentation ? themesOf(test).length : questionsOf(test).length,
                    );
                    return new TableRow({
                        cantSplit: true,
                        children: [
                            indexCell(String(idx + 1), INDEX_COLUMNS[0], AlignmentType.LEFT, {
                                bold: true,
                                color: ACCENT,
                            }),
                            indexCell(inline(test.title), INDEX_COLUMNS[1], AlignmentType.LEFT, {
                                bold: true,
                            }),
                            indexCell(
                                presentation ? labels.type_presentation : labels.type_test,
                                INDEX_COLUMNS[2],
                                AlignmentType.LEFT,
                                { color: MUTED },
                            ),
                            indexCell(volume, INDEX_COLUMNS[3], AlignmentType.RIGHT, { color: MUTED }),
                            indexCell(durationLabel(test, labels), INDEX_COLUMNS[4], AlignmentType.RIGHT, {
                                color: MUTED,
                            }),
                        ],
                    });
                }),
            ],
        }),
    ];

    // ---- one section body per test ----

    // A question is emitted as paragraph *specs* so the whole block can be glued
    // together with keepNext afterwards — a question that straddles a page break
    // separated from its own options is the one thing that ruins this layout.
    const questionBlock = (question: DocQuestion, number: number) => {
        const specs: IParagraphOptions[] = [];
        const points = question.points ?? 1;

        specs.push({
            spacing: { before: 280, after: 40, line: 300 },
            indent: { left: 460, hanging: 460 },
            children: [
                new TextRun({ text: `${number}.`, size: 22, bold: true, color: ACCENT, font: BODY_FONT }),
                new TextRun({ text: '\t', size: 22 }),
                new TextRun({
                    text: inline(question.question) || '—',
                    size: 22,
                    bold: true,
                    color: INK,
                    font: BODY_FONT,
                }),
            ],
        });

        const metaBits = [`${points} ${labels.points(points)}`];
        if (question.type === 'code') {
            metaBits.push(`${labels.code_task} · ${labels.language}: ${question.language || '—'}`);
        } else if (question.type === 'short_answer') {
            metaBits.push(labels.open_answer);
            if (question.min_words) metaBits.push(`${labels.min_words}: ${question.min_words}`);
            if (question.ai_grading) metaBits.push(labels.ai_grading);
        }
        specs.push({
            spacing: { after: 120 },
            indent: { left: 460 },
            children: [
                new TextRun({ text: metaBits.join('   ·   '), size: 17, color: MUTED, font: BODY_FONT }),
            ],
        });

        if (question.type === 'multiple_choice' && Array.isArray(question.options)) {
            question.options.forEach((option, optIdx) => {
                const isCorrect = optIdx === question.correct_answer;
                const letter = OPTION_LETTERS[optIdx] ?? '•';
                specs.push({
                    spacing: { after: 60, line: 280 },
                    indent: { left: 920, hanging: 340 },
                    children: [
                        new TextRun({
                            text: `${letter})`,
                            size: 20,
                            bold: isCorrect,
                            color: isCorrect ? CORRECT : MUTED,
                            font: BODY_FONT,
                        }),
                        new TextRun({ text: '\t', size: 20 }),
                        new TextRun({
                            text: inline(option),
                            size: 20,
                            bold: isCorrect,
                            color: isCorrect ? CORRECT : INK,
                            font: BODY_FONT,
                        }),
                        ...(isCorrect
                            ? [
                                  new TextRun({
                                      text: `   ✓ ${labels.answer_key}`,
                                      size: 17,
                                      bold: true,
                                      color: CORRECT,
                                      font: BODY_FONT,
                                  }),
                              ]
                            : []),
                    ],
                });
            });

            const explanation = inline(question.explanation);
            if (explanation) {
                specs.push({
                    spacing: { before: 100, after: 60, line: 280 },
                    indent: { left: 920 },
                    border: { left: { style: BorderStyle.SINGLE, size: 10, color: HAIRLINE, space: 10 } },
                    children: [
                        new TextRun({
                            text: `${labels.explanation}: `,
                            size: 18,
                            bold: true,
                            color: MUTED,
                            font: BODY_FONT,
                        }),
                        new TextRun({ text: explanation, size: 18, italics: true, color: MUTED, font: BODY_FONT }),
                    ],
                });
            }
        }

        if (question.type === 'short_answer') {
            const keywords = (question.expected_keywords ?? [])
                .map((keyword) => inline(keyword))
                .filter(Boolean);
            if (keywords.length > 0) {
                specs.push({
                    spacing: { after: 60, line: 280 },
                    indent: { left: 580 },
                    children: [
                        new TextRun({
                            text: `${labels.keywords}: `,
                            size: 18,
                            bold: true,
                            color: MUTED,
                            font: BODY_FONT,
                        }),
                        new TextRun({ text: keywords.join(', '), size: 18, color: CORRECT, font: BODY_FONT }),
                    ],
                });
            }
        }

        if (question.type === 'code') {
            const lines = (question.starter_code ?? '').replace(/\r/g, '').split('\n');
            if (lines.some((line) => line.trim().length > 0)) {
                const shaded = (children: InstanceType<typeof TextRun>[], line: number): IParagraphOptions => ({
                    spacing: { before: 0, after: 0, line },
                    indent: { left: 580, right: 200 },
                    shading: { type: ShadingType.CLEAR, fill: SURFACE, color: 'auto' },
                    children,
                });
                // Contiguous shaded paragraphs read as one block; the blank ones
                // at each end are the block's vertical padding.
                const pad = () => shaded([new TextRun({ text: '', size: 8, font: MONO_FONT })], 120);
                specs.push(pad());
                lines.forEach((line) =>
                    specs.push(
                        shaded([new TextRun({ text: line || ' ', size: 18, color: INK, font: MONO_FONT })], 260),
                    ),
                );
                specs.push(pad());
            }
        }

        // keepNext on everything but the tail keeps the question whole.
        return specs.map(
            (spec, idx) =>
                new Paragraph({ ...spec, keepLines: true, keepNext: idx < specs.length - 1 }),
        );
    };

    const testBlock = (test: Test, index: number) => {
        const nodes: Node[] = [];
        const presentation = isPresentation(test);
        const questions = questionsOf(test);
        const themes = themesOf(test);

        // In a catalogue every test opens on a fresh page, straight after the
        // contents. A lone test is already at the top of the document.
        if (!single) {
            nodes.push(new Paragraph({ spacing: { after: 0 }, children: [new PageBreak()] }));
        }

        const badges = [presentation ? labels.type_presentation : labels.type_test];
        if (test.is_active === false) badges.push(labels.archived);
        nodes.push(eyebrow(`${labels.brand}  ·  ${badges.join('  ·  ')}`, ACCENT, 80));

        nodes.push(
            new Paragraph({
                heading: HeadingLevel.HEADING_1,
                spacing: { before: 0, after: 100 },
                children: [
                    ...(single
                        ? []
                        : [
                              new TextRun({
                                  text: `${index + 1}.  `,
                                  size: 36,
                                  bold: true,
                                  color: ACCENT,
                                  font: BODY_FONT,
                              }),
                          ]),
                    new TextRun({ text: inline(test.title), size: 36, bold: true, color: INK, font: BODY_FONT }),
                ],
            }),
        );

        const description = block(test.description);
        if (description.length > 0) {
            description.forEach((line) =>
                nodes.push(body(line, { size: 20, color: MUTED, after: 60 })),
            );
        } else {
            nodes.push(body(labels.no_description, { size: 20, color: MUTED, italics: true, after: 60 }));
        }

        nodes.push(spacer(120));
        nodes.push(
            statStrip([
                { label: labels.duration, value: durationLabel(test, labels) },
                {
                    label: presentation ? labels.themes : labels.questions,
                    value: String(presentation ? themes.length : questions.length),
                },
                // passing_score arrives as a decimal, so "70.00" becomes "70".
                { label: labels.passing_score, value: `${Number(test.passing_score) || 0}%` },
                { label: labels.attempts, value: String(test.max_attempts || 1) },
            ]),
        );
        nodes.push(spacer(200));

        const instructions = block(test.instructions);
        if (instructions.length > 0) {
            nodes.push(calloutBox(labels.instructions, instructions));
            nodes.push(spacer(200));
        }

        if (presentation) {
            if (themes.length > 0) {
                nodes.push(eyebrow(labels.themes, MUTED, 120));
                themes.forEach((theme, idx) => {
                    nodes.push(
                        new Paragraph({
                            spacing: { after: 100, line: 300 },
                            indent: { left: 460, hanging: 460 },
                            children: [
                                new TextRun({
                                    text: `${idx + 1}.`,
                                    size: 21,
                                    bold: true,
                                    color: ACCENT,
                                    font: BODY_FONT,
                                }),
                                new TextRun({ text: '\t', size: 21 }),
                                new TextRun({ text: inline(theme), size: 21, color: INK, font: BODY_FONT }),
                            ],
                        }),
                    );
                });
            }

            const extra = block(test.presentation_extra_info);
            if (extra.length > 0) {
                nodes.push(spacer(120));
                nodes.push(calloutBox(labels.extra_info, extra));
            }
        } else if (questions.length === 0) {
            nodes.push(body(labels.no_questions, { size: 20, color: MUTED, italics: true }));
        } else {
            nodes.push(eyebrow(labels.section_questions, MUTED, 0));
            nodes.push(rule(HAIRLINE, false, 60));
            questions.forEach((question, idx) => {
                nodes.push(...questionBlock(question, idx + 1));
            });
        }

        return nodes;
    };

    const documentTitle = single
        ? `${inline(tests[0].title)} · ${formatDate(now, lang)}`
        : `${labels.cover_title} · ${formatDate(now, lang)}`;

    const contentHeader = new Header({
        children: [
            new Paragraph({
                alignment: AlignmentType.RIGHT,
                spacing: { after: 0 },
                border: { bottom: hairline },
                children: [
                    new TextRun({
                        text: documentTitle,
                        size: 15,
                        color: MUTED,
                        characterSpacing: 12,
                        font: BODY_FONT,
                    }),
                ],
            }),
        ],
    });

    const contentFooter = new Footer({
        children: [
            new Paragraph({
                alignment: AlignmentType.CENTER,
                spacing: { before: 0 },
                border: { top: hairline },
                children: [
                    new TextRun({ children: [PageNumber.CURRENT], size: 16, color: MUTED, font: BODY_FONT }),
                    new TextRun({ text: ' / ', size: 16, color: HAIRLINE, font: BODY_FONT }),
                    new TextRun({ children: [PageNumber.TOTAL_PAGES], size: 16, color: MUTED, font: BODY_FONT }),
                ],
            }),
        ],
    });

    const pageSetup = {
        margin: {
            top: convertMillimetersToTwip(22),
            right: convertMillimetersToTwip(20),
            bottom: convertMillimetersToTwip(20),
            left: convertMillimetersToTwip(20),
            header: convertMillimetersToTwip(12),
            footer: convertMillimetersToTwip(12),
        },
    };

    const doc = new Document({
        title: documentTitle,
        creator: labels.brand,
        description: labels.cover_subtitle,
        styles: {
            default: {
                document: {
                    run: { font: BODY_FONT, size: 21, color: INK },
                    paragraph: { spacing: { line: 300 } },
                },
            },
        },
        sections: single
            ? [
                  {
                      properties: { page: pageSetup },
                      headers: { default: contentHeader },
                      footers: { default: contentFooter },
                      children: tests.flatMap((test, idx) => testBlock(test, idx)),
                  },
              ]
            : [
                  // Cover — no running header/footer.
                  { properties: { page: pageSetup }, children: cover },
                  // Contents + every test. Numbering continues from the cover so the
                  // "current / total" pair in the footer stays consistent.
                  {
                      properties: { page: pageSetup },
                      headers: { default: contentHeader },
                      footers: { default: contentFooter },
                      children:
                          tests.length === 0
                              ? [...contents, spacer(200), body(labels.no_tests, { color: MUTED, italics: true })]
                              : [...contents, ...tests.flatMap((test, idx) => testBlock(test, idx))],
                  },
              ],
    });

    const blob = await Packer.toBlob(doc);
    return { blob, fileName: exportFileName(tests, lang, 'docx') };
}
