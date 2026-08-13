/**
 * Builds the Markdown export — the plain-text member of the family, meant for
 * wikis, pull requests and anything that renders GitHub-flavoured Markdown.
 *
 * Multiple-choice options become a task list so the correct answer is a ticked
 * box; code questions keep their starter code in a fenced block.
 */
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

const MD_MIME = 'text/markdown;charset=utf-8';

/**
 * Escapes the characters that would otherwise turn body text into markup.
 * List and heading markers only bite at the start of a line, so they are escaped
 * there and left alone mid-sentence — otherwise every hyphen grows a backslash.
 */
function escape(text: string): string {
    return text
        .replace(/([\\`*_[\]<>|])/g, '\\$1')
        .replace(/^(\s*)([#>+-]|\d+\.)(\s)/, '$1\\$2$3');
}

/** Quotes a multi-line passage as a Markdown blockquote. */
function quote(lines: string[]): string[] {
    return lines.map((line) => (line ? `> ${escape(line)}` : '>'));
}

function questionSection(question: DocQuestion, number: number, labels: Labels): string[] {
    const out: string[] = [];
    const points = question.points ?? 1;

    out.push(`**${number}.** ${escape(inline(question.question) || '—')}`);
    out.push('');

    const meta = [`${points} ${labels.points(points)}`];
    if (question.type === 'code') {
        meta.push(`${labels.code_task} · ${labels.language}: ${question.language || '—'}`);
    } else if (question.type === 'short_answer') {
        meta.push(labels.open_answer);
        if (question.min_words) meta.push(`${labels.min_words}: ${question.min_words}`);
        if (question.ai_grading) meta.push(labels.ai_grading);
    }
    out.push(`_${escape(meta.join(' · '))}_`);
    out.push('');

    if (question.type === 'multiple_choice' && Array.isArray(question.options)) {
        question.options.forEach((option, idx) => {
            const isCorrect = idx === question.correct_answer;
            const letter = OPTION_LETTERS[idx] ?? '•';
            const mark = isCorrect ? 'x' : ' ';
            const suffix = isCorrect ? ` ✅ _${escape(labels.answer_key)}_` : '';
            out.push(`- [${mark}] **${letter})** ${escape(inline(option))}${suffix}`);
        });
        out.push('');

        const explanation = inline(question.explanation);
        if (explanation) {
            out.push(`> **${escape(labels.explanation)}:** ${escape(explanation)}`);
            out.push('');
        }
    }

    if (question.type === 'short_answer') {
        const keywords = (question.expected_keywords ?? []).map((k) => inline(k)).filter(Boolean);
        if (keywords.length > 0) {
            out.push(`**${escape(labels.keywords)}:** ${keywords.map((k) => `\`${k}\``).join(', ')}`);
            out.push('');
        }
    }

    if (question.type === 'code') {
        const starter = (question.starter_code ?? '').replace(/\r/g, '').replace(/\s+$/, '');
        if (starter.trim().length > 0) {
            out.push('```' + (question.language || ''));
            out.push(starter);
            out.push('```');
            out.push('');
        }
    }

    return out;
}

function testSection(test: Test, number: number | null, labels: Labels): string[] {
    const out: string[] = [];
    const presentation = isPresentation(test);
    const questions = questionsOf(test);
    const themes = themesOf(test);

    const badges = [presentation ? labels.type_presentation : labels.type_test];
    if (test.is_active === false) badges.push(labels.archived);

    const heading = number === null ? inline(test.title) : `${number}. ${inline(test.title)}`;
    // A catalogue nests its tests under the document title; a lone test owns it.
    out.push(`${number === null ? '#' : '##'} ${escape(heading)}`);
    out.push('');
    out.push(`_${escape(badges.join(' · '))}_`);
    out.push('');

    const description = block(test.description);
    out.push(...quote(description.length > 0 ? description : [labels.no_description]));
    out.push('');

    out.push(
        `| ${escape(labels.duration)} | ${escape(presentation ? labels.themes : labels.questions)} | ${escape(labels.passing_score)} | ${escape(labels.attempts)} |`,
    );
    out.push('| --- | --- | --- | --- |');
    out.push(
        `| ${escape(durationLabel(test, labels))} | ${presentation ? themes.length : questions.length} | ${Number(test.passing_score) || 0}% | ${test.max_attempts || 1} |`,
    );
    out.push('');

    const instructions = block(test.instructions);
    if (instructions.length > 0) {
        out.push(`${number === null ? '##' : '###'} ${escape(labels.instructions)}`);
        out.push('');
        out.push(...instructions.map((line) => escape(line)));
        out.push('');
    }

    if (presentation) {
        if (themes.length > 0) {
            out.push(`${number === null ? '##' : '###'} ${escape(labels.themes)}`);
            out.push('');
            themes.forEach((theme, idx) => out.push(`${idx + 1}. ${escape(inline(theme))}`));
            out.push('');
        }
        const extra = block(test.presentation_extra_info);
        if (extra.length > 0) {
            out.push(`${number === null ? '##' : '###'} ${escape(labels.extra_info)}`);
            out.push('');
            out.push(...extra.map((line) => escape(line)));
            out.push('');
        }
    } else if (questions.length === 0) {
        out.push(`_${escape(labels.no_questions)}_`);
        out.push('');
    } else {
        out.push(`${number === null ? '##' : '###'} ${escape(labels.section_questions)}`);
        out.push('');
        questions.forEach((question, idx) => {
            out.push(...questionSection(question, idx + 1, labels));
        });
    }

    // Nothing below the last section of a standalone test needs a rule.
    if (number !== null) {
        out.push('---');
        out.push('');
    }

    return out;
}

/** Renders the given tests as a Markdown document. */
export function buildTestsMarkdown(tests: Test[], lang: string): string {
    const labels = labelsFor(lang);
    const single = tests.length === 1;
    const now = new Date();

    if (single) {
        return testSection(tests[0], null, labels).join('\n').replace(/\n{3,}/g, '\n\n').trimEnd() + '\n';
    }

    const totalQuestions = tests.reduce((sum, test) => sum + questionsOf(test).length, 0);
    const totalMinutes = tests.reduce((sum, test) => sum + (test.duration_minutes || 0), 0);

    const out: string[] = [
        `# ${escape(labels.cover_title)}`,
        '',
        `_${escape(labels.cover_subtitle)}_`,
        '',
        `**${escape(labels.generated)}:** ${escape(formatDate(now, lang))}`,
        '',
        `| ${escape(labels.total_tests)} | ${escape(labels.total_questions)} | ${escape(labels.total_duration)} |`,
        '| --- | --- | --- |',
        `| ${tests.length} | ${totalQuestions} | ${escape(totalDurationLabel(totalMinutes, labels))} |`,
        '',
    ];

    if (tests.length === 0) {
        out.push(`_${escape(labels.no_tests)}_`, '');
        return out.join('\n');
    }

    out.push(`## ${escape(labels.contents)}`, '');
    out.push(
        `| ${escape(labels.index_no)} | ${escape(labels.index_title)} | ${escape(labels.index_type)} | ${escape(labels.index_volume)} | ${escape(labels.index_duration)} |`,
    );
    out.push('| --- | --- | --- | --- | --- |');
    tests.forEach((test, idx) => {
        const presentation = isPresentation(test);
        const volume = presentation ? themesOf(test).length : questionsOf(test).length;
        out.push(
            `| ${idx + 1} | ${escape(inline(test.title))} | ${escape(presentation ? labels.type_presentation : labels.type_test)} | ${volume} | ${escape(durationLabel(test, labels))} |`,
        );
    });
    out.push('', '---', '');

    tests.forEach((test, idx) => {
        out.push(...testSection(test, idx + 1, labels));
    });

    return out.join('\n').replace(/\n{3,}/g, '\n\n').trimEnd() + '\n';
}

/** Renders the Markdown export, plus the file name it should be saved under. */
export function buildTestsMarkdownFile(
    tests: Test[],
    lang: string,
): { blob: Blob; fileName: string } {
    const markdown = buildTestsMarkdown(tests, lang);
    return {
        blob: new Blob([markdown], { type: MD_MIME }),
        fileName: exportFileName(tests, lang, 'md'),
    };
}
