/**
 * Pieces every test export shares: the bilingual label set, the text sanitisers
 * the rich-text editor makes necessary, and the small accessors that read a
 * loosely-typed `Test` safely.
 *
 * The Word, Excel and Markdown writers all build on this so the four exports
 * (PDF is rendered by the backend) describe a test the same way.
 */
import { Test } from '@/types/api';
import { formatText } from '../utils';

export const OPTION_LETTERS = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L'];

/** Questions arrive as loose JSON: the details variant is flattened onto the object. */
export interface DocQuestion {
    type?: 'multiple_choice' | 'code' | 'short_answer';
    question?: string;
    points?: number;
    options?: string[];
    correct_answer?: number;
    explanation?: string | null;
    expected_keywords?: string[] | null;
    min_words?: number | null;
    ai_grading?: boolean;
    language?: string;
    starter_code?: string | null;
}

export interface Labels {
    brand: string;
    cover_title: string;
    cover_subtitle: string;
    generated: string;
    contents: string;
    index_no: string;
    index_title: string;
    index_type: string;
    index_volume: string;
    index_duration: string;
    total_tests: string;
    total_questions: string;
    total_duration: string;
    type_test: string;
    type_presentation: string;
    duration: string;
    minutes: string;
    hours: string;
    questions: string;
    section_questions: string;
    passing_score: string;
    attempts: string;
    instructions: string;
    themes: string;
    theme: string;
    extra_info: string;
    answer_key: string;
    open_answer: string;
    multiple_choice: string;
    min_words: string;
    keywords: string;
    ai_grading: string;
    code_task: string;
    language: string;
    starter_code: string;
    explanation: string;
    archived: string;
    active: string;
    status: string;
    created: string;
    description: string;
    no_questions: string;
    no_description: string;
    no_tests: string;
    filename: string;
    test: string;
    number: string;
    question: string;
    type: string;
    points_column: string;
    correct: string;
    details: string;
    sheet_overview: string;
    sheet_questions: string;
    sheet_themes: string;
    points: (n: number) => string;
}

const RU: Labels = {
    brand: 'KoinotHR',
    cover_title: 'Каталог тестов',
    cover_subtitle: 'Полный сборник оценочных материалов',
    generated: 'Сформировано',
    contents: 'Содержание',
    index_no: '№',
    index_title: 'Название',
    index_type: 'Тип',
    index_volume: 'Объём',
    index_duration: 'Время',
    total_tests: 'Тестов',
    total_questions: 'Вопросов',
    total_duration: 'Общее время',
    type_test: 'Тест',
    type_presentation: 'Презентация',
    duration: 'Длительность',
    minutes: 'мин',
    hours: 'ч',
    questions: 'Вопросов',
    section_questions: 'Вопросы',
    passing_score: 'Проходной балл',
    attempts: 'Попыток',
    instructions: 'Инструкция',
    themes: 'Темы презентации',
    theme: 'Тема',
    extra_info: 'Дополнительно',
    answer_key: 'верный ответ',
    open_answer: 'Развёрнутый ответ',
    multiple_choice: 'Выбор ответа',
    min_words: 'минимум слов',
    keywords: 'Ключевые слова',
    ai_grading: 'Проверка ИИ',
    code_task: 'Задача на код',
    language: 'Язык',
    starter_code: 'Заготовка кода',
    explanation: 'Пояснение',
    archived: 'В архиве',
    active: 'Активен',
    status: 'Статус',
    created: 'Создан',
    description: 'Описание',
    no_questions: 'В тесте нет вопросов',
    no_description: 'Описание отсутствует',
    no_tests: 'Тесты не найдены',
    filename: 'Каталог тестов',
    test: 'Тест',
    number: '№',
    question: 'Вопрос',
    type: 'Тип',
    points_column: 'Баллы',
    correct: 'Верный ответ',
    details: 'Детали',
    sheet_overview: 'Тесты',
    sheet_questions: 'Вопросы',
    sheet_themes: 'Темы презентаций',
    points: (n) => {
        const abs = Math.abs(n);
        if (abs % 100 >= 11 && abs % 100 <= 14) return 'баллов';
        const last = abs % 10;
        if (last === 1) return 'балл';
        if (last >= 2 && last <= 4) return 'балла';
        return 'баллов';
    },
};

const EN: Labels = {
    brand: 'KoinotHR',
    cover_title: 'Test Catalogue',
    cover_subtitle: 'Complete collection of assessment materials',
    generated: 'Generated',
    contents: 'Contents',
    index_no: '#',
    index_title: 'Title',
    index_type: 'Type',
    index_volume: 'Volume',
    index_duration: 'Time',
    total_tests: 'Tests',
    total_questions: 'Questions',
    total_duration: 'Total time',
    type_test: 'Test',
    type_presentation: 'Presentation',
    duration: 'Duration',
    minutes: 'min',
    hours: 'h',
    questions: 'Questions',
    section_questions: 'Questions',
    passing_score: 'Passing score',
    attempts: 'Attempts',
    instructions: 'Instructions',
    themes: 'Presentation topics',
    theme: 'Topic',
    extra_info: 'Additional info',
    answer_key: 'correct answer',
    open_answer: 'Open answer',
    multiple_choice: 'Multiple choice',
    min_words: 'minimum words',
    keywords: 'Keywords',
    ai_grading: 'AI grading',
    code_task: 'Coding task',
    language: 'Language',
    starter_code: 'Starter code',
    explanation: 'Explanation',
    archived: 'Archived',
    active: 'Active',
    status: 'Status',
    created: 'Created',
    description: 'Description',
    no_questions: 'This test has no questions',
    no_description: 'No description',
    no_tests: 'No tests found',
    filename: 'Test catalogue',
    test: 'Test',
    number: '#',
    question: 'Question',
    type: 'Type',
    points_column: 'Points',
    correct: 'Correct answer',
    details: 'Details',
    sheet_overview: 'Tests',
    sheet_questions: 'Questions',
    sheet_themes: 'Presentation topics',
    points: (n) => (Math.abs(n) === 1 ? 'point' : 'points'),
};

export function labelsFor(lang: string): Labels {
    return lang === 'en' ? EN : RU;
}

// ---- text helpers ----

/** Strips markup and entities the rich-text editor may have left behind. */
export function stripHtml(input: string): string {
    let text = input.replace(/<br\s*\/?>/gi, '\n').replace(/<\/(p|div|li)>/gi, '\n');
    text = text.replace(/<[^>]*>/g, '');
    return text
        .replace(/&nbsp;/g, ' ')
        .replace(/&quot;/g, '"')
        .replace(/&#39;/g, "'")
        .replace(/&lt;/g, '<')
        .replace(/&gt;/g, '>')
        .replace(/&amp;/g, '&');
}

/** Single-line text: collapses every run of whitespace. */
export function inline(input: string | null | undefined): string {
    if (!input) return '';
    return stripHtml(formatText(input)).replace(/\s+/g, ' ').trim();
}

/** Multi-line text: keeps paragraph breaks, tidies everything else. */
export function block(input: string | null | undefined): string[] {
    if (!input) return [];
    return stripHtml(formatText(input))
        .split('\n')
        .map((line) => line.replace(/[^\S\n]+/g, ' ').trim())
        .filter((line, idx, all) => line.length > 0 || (idx > 0 && all[idx - 1].length > 0))
        .filter((line, idx, all) => !(idx === all.length - 1 && line.length === 0));
}

// ---- test accessors ----

export function questionsOf(test: Test): DocQuestion[] {
    return Array.isArray(test.questions) ? (test.questions as DocQuestion[]) : [];
}

export function themesOf(test: Test): string[] {
    return Array.isArray(test.presentation_themes) ? test.presentation_themes : [];
}

export function isPresentation(test: Test): boolean {
    return test.test_type === 'presentation';
}

export function durationLabel(test: Test, labels: Labels): string {
    return isPresentation(test)
        ? `${Math.round((test.duration_minutes / 60) * 10) / 10} ${labels.hours}`
        : `${test.duration_minutes} ${labels.minutes}`;
}

/** Turns a minute count into "2 ч 30 мин" / "2 h 30 min", dropping empty parts. */
export function totalDurationLabel(minutes: number, labels: Labels): string {
    const hours = Math.floor(minutes / 60);
    const rest = minutes % 60;
    if (hours === 0) return `${rest} ${labels.minutes}`;
    if (rest === 0) return `${hours} ${labels.hours}`;
    return `${hours} ${labels.hours} ${rest} ${labels.minutes}`;
}

/** Human label for a question's kind, used by the flat exports. */
export function questionTypeLabel(question: DocQuestion, labels: Labels): string {
    switch (question.type) {
        case 'code':
            return labels.code_task;
        case 'short_answer':
            return labels.open_answer;
        default:
            return labels.multiple_choice;
    }
}

export function formatDate(date: Date, lang: string): string {
    return date.toLocaleDateString(lang === 'en' ? 'en-GB' : 'ru-RU', {
        day: '2-digit',
        month: 'long',
        year: 'numeric',
    });
}

/** Windows and Word both choke on \ / : * ? " < > | in file names. */
export function safeFileName(name: string): string {
    return name.replace(/[\\/:*?"<>|]/g, '-').replace(/\s+/g, ' ').trim();
}

/** ISO date suffix shared by every catalogue file name. */
export function fileDateStamp(date: Date): string {
    return date.toISOString().slice(0, 10);
}

/**
 * The name a download gets: the test's own title for a single test, the
 * catalogue plus today's date for the whole library.
 */
export function exportFileName(tests: Test[], lang: string, extension: string): string {
    const labels = labelsFor(lang);
    if (tests.length === 1) {
        const title = inline(tests[0].title) || labels.test;
        return safeFileName(`${title}.${extension}`);
    }
    return safeFileName(`${labels.filename} ${fileDateStamp(new Date())}.${extension}`);
}
