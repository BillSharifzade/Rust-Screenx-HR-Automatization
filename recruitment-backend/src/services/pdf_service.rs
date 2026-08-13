use crate::error::{Error, Result};
use crate::models::question::{Question, QuestionDetails, QuestionType};
use crate::models::test::Test;
use genpdf::style::{Color, Style};
use genpdf::{elements, fonts, Alignment, Element, Margins, SimplePageDecorator};

const FONT_REGULAR: &[u8] = include_bytes!("../../assets/fonts/LiberationSans-Regular.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../../assets/fonts/LiberationSans-Bold.ttf");

const OPTION_LETTERS: [&str; 12] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L",
];

const INK: Color = Color::Rgb(17, 24, 39);
const MUTED: Color = Color::Rgb(107, 114, 128);
const ACCENT: Color = Color::Rgb(29, 78, 216);
const CORRECT: Color = Color::Rgb(21, 128, 61);

pub struct PdfService;

struct Labels {
    brand: &'static str,
    duration: &'static str,
    minutes: &'static str,
    hours: &'static str,
    questions: &'static str,
    passing_score: &'static str,
    attempts: &'static str,
    instructions: &'static str,
    themes: &'static str,
    extra_info: &'static str,
    answer_key: &'static str,
    open_answer: &'static str,
    min_words: &'static str,
    keywords: &'static str,
    code_task: &'static str,
    language: &'static str,
    explanation: &'static str,
    points_one: &'static str,
    points_few: &'static str,
    points_many: &'static str,
    page: &'static str,
    no_questions: &'static str,
    catalogue_title: &'static str,
    catalogue_subtitle: &'static str,
    generated: &'static str,
    contents: &'static str,
    index_no: &'static str,
    index_title: &'static str,
    index_type: &'static str,
    index_volume: &'static str,
    total_tests: &'static str,
    total_questions: &'static str,
    total_duration: &'static str,
    type_test: &'static str,
    type_presentation: &'static str,
    archived: &'static str,
    no_tests: &'static str,
}

impl Labels {
    fn points(&self, n: i32) -> &'static str {
        let abs = n.unsigned_abs();
        if abs % 100 >= 11 && abs % 100 <= 14 {
            return self.points_many;
        }
        match abs % 10 {
            1 => self.points_one,
            2..=4 => self.points_few,
            _ => self.points_many,
        }
    }
}

const RU: Labels = Labels {
    brand: "KoinotHR",
    duration: "Длительность",
    minutes: "мин",
    hours: "ч",
    questions: "Вопросов",
    passing_score: "Проходной балл",
    attempts: "Попыток",
    instructions: "Инструкция",
    themes: "Темы презентации",
    extra_info: "Дополнительно",
    answer_key: "верный ответ",
    open_answer: "Развёрнутый ответ",
    min_words: "минимум слов",
    keywords: "Ключевые слова",
    code_task: "Задача на код",
    language: "Язык",
    explanation: "Пояснение",
    points_one: "балл",
    points_few: "балла",
    points_many: "баллов",
    page: "Стр.",
    no_questions: "В тесте нет вопросов",
    catalogue_title: "Каталог тестов",
    catalogue_subtitle: "Полный сборник оценочных материалов",
    generated: "Сформировано",
    contents: "Содержание",
    index_no: "№",
    index_title: "Название",
    index_type: "Тип",
    index_volume: "Объём",
    total_tests: "Тестов",
    total_questions: "Вопросов",
    total_duration: "Общее время",
    type_test: "Тест",
    type_presentation: "Презентация",
    archived: "В архиве",
    no_tests: "Тесты не найдены",
};

const EN: Labels = Labels {
    brand: "KoinotHR",
    duration: "Duration",
    minutes: "min",
    hours: "h",
    questions: "Questions",
    passing_score: "Passing score",
    attempts: "Attempts",
    instructions: "Instructions",
    themes: "Presentation topics",
    extra_info: "Additional info",
    answer_key: "correct answer",
    open_answer: "Open answer",
    min_words: "minimum words",
    keywords: "Keywords",
    code_task: "Coding task",
    language: "Language",
    explanation: "Explanation",
    points_one: "point",
    points_few: "points",
    points_many: "points",
    page: "Page",
    no_questions: "This test has no questions",
    catalogue_title: "Test Catalogue",
    catalogue_subtitle: "Complete collection of assessment materials",
    generated: "Generated",
    contents: "Contents",
    index_no: "#",
    index_title: "Title",
    index_type: "Type",
    index_volume: "Volume",
    total_tests: "Tests",
    total_questions: "Questions",
    total_duration: "Total time",
    type_test: "Test",
    type_presentation: "Presentation",
    archived: "Archived",
    no_tests: "No tests found",
};

impl PdfService {
    pub fn generate_test_pdf(test: &Test, lang: &str) -> Result<Vec<u8>> {
        let labels = labels_for(lang);
        let mut doc = Self::new_document(&transliterate(&strip_html(&test.title)), labels)?;

        Self::push_test(&mut doc, test, None, labels);

        Self::render(doc)
    }

    pub fn generate_tests_catalogue_pdf(tests: &[Test], lang: &str) -> Result<Vec<u8>> {
        let labels = labels_for(lang);
        let mut doc = Self::new_document(labels.catalogue_title, labels)?;

        Self::push_cover(&mut doc, tests, labels);

        if tests.is_empty() {
            doc.push(
                elements::Paragraph::new(labels.no_tests)
                    .styled(Style::new().italic().with_font_size(10).with_color(MUTED)),
            );
            return Self::render(doc);
        }

        Self::push_contents(&mut doc, tests, labels)?;

        for (idx, test) in tests.iter().enumerate() {
            doc.push(elements::PageBreak::new());
            Self::push_test(&mut doc, test, Some(idx + 1), labels);
        }

        Self::render(doc)
    }

    fn new_document(title: &str, labels: &Labels) -> Result<genpdf::Document> {
        let regular = fonts::FontData::new(FONT_REGULAR.to_vec(), None)
            .map_err(|e| Error::Internal(format!("font load failed: {}", e)))?;
        let bold = fonts::FontData::new(FONT_BOLD.to_vec(), None)
            .map_err(|e| Error::Internal(format!("font load failed: {}", e)))?;
        let italic = fonts::FontData::new(FONT_REGULAR.to_vec(), None)
            .map_err(|e| Error::Internal(format!("font load failed: {}", e)))?;
        let bold_italic = fonts::FontData::new(FONT_BOLD.to_vec(), None)
            .map_err(|e| Error::Internal(format!("font load failed: {}", e)))?;

        let family = fonts::FontFamily { regular, bold, italic, bold_italic };

        let mut doc = genpdf::Document::new(family);
        doc.set_title(transliterate(title));
        doc.set_minimal_conformance();
        doc.set_font_size(10);
        doc.set_line_spacing(1.35);

        let page_label = labels.page;
        let mut decorator = SimplePageDecorator::new();
        decorator.set_margins(Margins::trbl(18, 20, 20, 20));
        decorator.set_header(move |page| {
            let mut layout = elements::LinearLayout::vertical();
            if page > 1 {
                layout.push(
                    elements::Paragraph::new(format!("{} {}", page_label, page))
                        .aligned(Alignment::Right)
                        .styled(Style::new().with_font_size(8).with_color(MUTED)),
                );
                layout.push(elements::Break::new(0.6));
            }
            layout
        });
        doc.set_page_decorator(decorator);

        Ok(doc)
    }

    fn render(doc: genpdf::Document) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        doc.render(&mut buffer)
            .map_err(|e| Error::Internal(format!("pdf render failed: {}", e)))?;
        Ok(buffer)
    }

    fn push_test(doc: &mut genpdf::Document, test: &Test, number: Option<usize>, labels: &Labels) {
        Self::push_header(doc, test, number, labels);

        let is_presentation = test.test_type.as_deref() == Some("presentation");
        if is_presentation {
            Self::push_presentation_body(doc, test, labels);
        } else {
            Self::push_questions(doc, test, labels);
        }
    }

    fn push_cover(doc: &mut genpdf::Document, tests: &[Test], labels: &Labels) {
        let total_questions: usize = tests.iter().map(|t| questions_of(t).len()).sum();
        let total_minutes: i32 = tests.iter().map(|t| t.duration_minutes).sum();

        doc.push(
            elements::Paragraph::new(labels.brand.to_uppercase())
                .styled(Style::new().bold().with_font_size(9).with_color(ACCENT)),
        );
        doc.push(elements::Break::new(0.4));
        doc.push(
            elements::Paragraph::new(labels.catalogue_title)
                .styled(Style::new().bold().with_font_size(26).with_color(INK)),
        );
        doc.push(elements::Break::new(0.3));
        doc.push(
            elements::Paragraph::new(labels.catalogue_subtitle)
                .styled(Style::new().with_font_size(11).with_color(MUTED)),
        );
        doc.push(elements::Break::new(0.3));
        doc.push(
            elements::Paragraph::new(format!(
                "{}: {}",
                labels.generated,
                chrono::Utc::now().format("%d.%m.%Y")
            ))
            .styled(Style::new().with_font_size(9).with_color(MUTED)),
        );
        doc.push(elements::Break::new(0.8));
        doc.push(
            elements::Paragraph::new(format!(
                "{}: {}   |   {}: {}   |   {}: {}",
                labels.total_tests,
                tests.len(),
                labels.total_questions,
                total_questions,
                labels.total_duration,
                total_duration(total_minutes, labels),
            ))
            .styled(Style::new().with_font_size(9).with_color(ACCENT))
            .padded(Margins::trbl(2, 0, 2, 0))
            .framed(),
        );
        doc.push(elements::Break::new(1.0));
    }

    fn push_contents(doc: &mut genpdf::Document, tests: &[Test], labels: &Labels) -> Result<()> {
        doc.push(
            elements::Paragraph::new(labels.contents)
                .styled(Style::new().bold().with_font_size(14).with_color(INK)),
        );
        doc.push(elements::Break::new(0.5));

        let header = Style::new().bold().with_font_size(8).with_color(MUTED);
        let cell = Style::new().with_font_size(9).with_color(INK);
        let muted_cell = Style::new().with_font_size(9).with_color(MUTED);

        let mut table = elements::TableLayout::new(vec![1, 9, 3, 2, 2]);
        table.set_cell_decorator(elements::FrameCellDecorator::new(false, true, false));

        table
            .row()
            .element(
                elements::Paragraph::new(labels.index_no.to_uppercase())
                    .styled(header)
                    .padded(Margins::trbl(1, 2, 1, 0)),
            )
            .element(
                elements::Paragraph::new(labels.index_title.to_uppercase())
                    .styled(header)
                    .padded(Margins::trbl(1, 2, 1, 0)),
            )
            .element(
                elements::Paragraph::new(labels.index_type.to_uppercase())
                    .styled(header)
                    .padded(Margins::trbl(1, 2, 1, 0)),
            )
            .element(
                elements::Paragraph::new(labels.index_volume.to_uppercase())
                    .styled(header)
                    .padded(Margins::trbl(1, 2, 1, 0)),
            )
            .element(
                elements::Paragraph::new(labels.duration.to_uppercase())
                    .styled(header)
                    .padded(Margins::trbl(1, 2, 1, 0)),
            )
            .push()
            .map_err(|e| Error::Internal(format!("pdf table failed: {}", e)))?;

        for (idx, test) in tests.iter().enumerate() {
            let is_presentation = test.test_type.as_deref() == Some("presentation");
            let volume = if is_presentation {
                themes_of(test).len()
            } else {
                questions_of(test).len()
            };
            let kind = if is_presentation {
                labels.type_presentation
            } else {
                labels.type_test
            };

            table
                .row()
                .element(
                    elements::Paragraph::new(format!("{}", idx + 1))
                        .styled(Style::new().bold().with_font_size(9).with_color(ACCENT))
                        .padded(Margins::trbl(1, 2, 1, 0)),
                )
                .element(
                    elements::Paragraph::new(strip_html(&test.title))
                        .styled(cell)
                        .padded(Margins::trbl(1, 2, 1, 0)),
                )
                .element(
                    elements::Paragraph::new(kind)
                        .styled(muted_cell)
                        .padded(Margins::trbl(1, 2, 1, 0)),
                )
                .element(
                    elements::Paragraph::new(format!("{}", volume))
                        .styled(muted_cell)
                        .padded(Margins::trbl(1, 2, 1, 0)),
                )
                .element(
                    elements::Paragraph::new(duration_label(test, labels))
                        .styled(muted_cell)
                        .padded(Margins::trbl(1, 2, 1, 0)),
                )
                .push()
                .map_err(|e| Error::Internal(format!("pdf table failed: {}", e)))?;
        }

        doc.push(table);
        Ok(())
    }

    fn push_header(doc: &mut genpdf::Document, test: &Test, number: Option<usize>, labels: &Labels) {
        let is_presentation = test.test_type.as_deref() == Some("presentation");
        let mut badges = vec![if is_presentation {
            labels.type_presentation
        } else {
            labels.type_test
        }
        .to_string()];
        if test.is_active == Some(false) {
            badges.push(labels.archived.to_string());
        }
        doc.push(
            elements::Paragraph::new(format!("{}  ·  {}", labels.brand, badges.join("  ·  ")).to_uppercase())
                .styled(Style::new().bold().with_font_size(8).with_color(ACCENT)),
        );
        doc.push(elements::Break::new(0.3));

        let heading = match number {
            Some(n) => format!("{}. {}", n, strip_html(&test.title)),
            None => strip_html(&test.title),
        };
        doc.push(
            elements::Paragraph::new(heading)
                .styled(Style::new().bold().with_font_size(20).with_color(INK)),
        );
        doc.push(elements::Break::new(0.3));

        let description = test.description.as_deref().map(strip_html).unwrap_or_default();
        if !description.is_empty() {
            doc.push(
                elements::Paragraph::new(description)
                    .styled(Style::new().with_font_size(10).with_color(MUTED)),
            );
            doc.push(elements::Break::new(0.4));
        }

        doc.push(
            elements::Paragraph::new(Self::meta_line(test, labels))
                .styled(Style::new().with_font_size(9).with_color(ACCENT))
                .padded(Margins::trbl(2, 0, 2, 0))
                .framed(),
        );
        doc.push(elements::Break::new(0.8));

        let instructions = test.instructions.as_deref().map(strip_html).unwrap_or_default();
        if !instructions.is_empty() {
            doc.push(
                elements::Paragraph::new(format!("{}: {}", labels.instructions, instructions))
                    .styled(Style::new().with_font_size(9).with_color(MUTED)),
            );
            doc.push(elements::Break::new(0.8));
        }
    }

    fn meta_line(test: &Test, labels: &Labels) -> String {
        let is_presentation = test.test_type.as_deref() == Some("presentation");
        let duration = duration_label(test, labels);

        let count = if is_presentation {
            themes_of(test).len()
        } else {
            questions_of(test).len()
        };
        let count_label = if is_presentation { labels.themes } else { labels.questions };

        format!(
            "{}: {}   |   {}: {}   |   {}: {}%   |   {}: {}",
            labels.duration,
            duration,
            count_label,
            count,
            labels.passing_score,
            test.passing_score.normalize(),
            labels.attempts,
            test.max_attempts.unwrap_or(1),
        )
    }

    fn push_presentation_body(doc: &mut genpdf::Document, test: &Test, labels: &Labels) {
        let themes = themes_of(test);
        if !themes.is_empty() {
            doc.push(
                elements::Paragraph::new(labels.themes)
                    .styled(Style::new().bold().with_font_size(12).with_color(INK)),
            );
            doc.push(elements::Break::new(0.4));

            for (idx, theme) in themes.iter().enumerate() {
                doc.push(
                    elements::Paragraph::new(format!("{}. {}", idx + 1, strip_html(theme)))
                        .styled(Style::new().with_font_size(10).with_color(INK))
                        .padded(Margins::trbl(0, 0, 2, 4)),
                );
            }
            doc.push(elements::Break::new(0.8));
        }

        let extra = test.presentation_extra_info.as_deref().map(strip_html).unwrap_or_default();
        if !extra.is_empty() {
            doc.push(
                elements::Paragraph::new(format!("{}: {}", labels.extra_info, extra))
                    .styled(Style::new().with_font_size(9).with_color(MUTED)),
            );
        }
    }

    fn push_questions(doc: &mut genpdf::Document, test: &Test, labels: &Labels) {
        let questions = questions_of(test);
        if questions.is_empty() {
            doc.push(
                elements::Paragraph::new(labels.no_questions)
                    .styled(Style::new().italic().with_font_size(10).with_color(MUTED)),
            );
            return;
        }

        for (idx, question) in questions.iter().enumerate() {
            Self::push_question(doc, idx + 1, question, labels);
        }
    }

    fn push_question(doc: &mut genpdf::Document, number: usize, question: &Question, labels: &Labels) {
        let heading = Style::new().bold().with_font_size(11).with_color(INK);
        doc.push(
            elements::Paragraph::new(format!("{}. ", number))
                .styled_string(strip_html(&question.question), heading)
                .styled(heading),
        );

        let points = Style::new().with_font_size(8).with_color(MUTED);
        doc.push(
            elements::Paragraph::new(format!("{} {}", question.points, labels.points(question.points)))
                .styled(points)
                .padded(Margins::trbl(0, 0, 1, 4)),
        );

        match &question.details {
            QuestionDetails::MultipleChoice(mc) => {
                for (opt_idx, option) in mc.options.iter().enumerate() {
                    let letter = OPTION_LETTERS
                        .get(opt_idx)
                        .copied()
                        .unwrap_or("•")
                        .to_string();
                    let is_correct = opt_idx as i32 == mc.correct_answer;
                    let style = if is_correct {
                        Style::new().bold().with_font_size(10).with_color(CORRECT)
                    } else {
                        Style::new().with_font_size(10).with_color(INK)
                    };
                    let suffix = if is_correct {
                        format!("   ({})", labels.answer_key)
                    } else {
                        String::new()
                    };
                    doc.push(
                        elements::Paragraph::new(format!(
                            "{}) {}{}",
                            letter,
                            strip_html(option),
                            suffix
                        ))
                        .styled(style)
                        .padded(Margins::trbl(0, 0, 1, 8)),
                    );
                }

                if let Some(explanation) = mc.explanation.as_deref() {
                    let text = strip_html(explanation);
                    if !text.is_empty() {
                        doc.push(
                            elements::Paragraph::new(format!("{}: {}", labels.explanation, text))
                                .styled(
                                    Style::new().italic().with_font_size(9).with_color(MUTED),
                                )
                                .padded(Margins::trbl(1, 0, 0, 8)),
                        );
                    }
                }
            }
            QuestionDetails::ShortAnswer(sa) => {
                let mut parts = vec![labels.open_answer.to_string()];
                if let Some(min_words) = sa.min_words {
                    parts.push(format!("{}: {}", labels.min_words, min_words));
                }
                doc.push(
                    elements::Paragraph::new(parts.join("   |   "))
                        .styled(Style::new().italic().with_font_size(9).with_color(MUTED))
                        .padded(Margins::trbl(0, 0, 1, 8)),
                );

                if let Some(keywords) = sa.expected_keywords.as_ref() {
                    let joined = keywords
                        .iter()
                        .map(|k| strip_html(k))
                        .filter(|k| !k.is_empty())
                        .collect::<Vec<_>>()
                        .join(", ");
                    if !joined.is_empty() {
                        doc.push(
                            elements::Paragraph::new(format!("{}: {}", labels.keywords, joined))
                                .styled(Style::new().with_font_size(9).with_color(CORRECT))
                                .padded(Margins::trbl(0, 0, 1, 8)),
                        );
                    }
                }
            }
            QuestionDetails::Code(code) => {
                doc.push(
                    elements::Paragraph::new(format!(
                        "{}   |   {}: {}",
                        labels.code_task, labels.language, code.language
                    ))
                    .styled(Style::new().italic().with_font_size(9).with_color(MUTED))
                    .padded(Margins::trbl(0, 0, 1, 8)),
                );

                if let Some(starter) = code.starter_code.as_deref() {
                    for line in starter.lines() {
                        doc.push(
                            elements::Paragraph::new(line.to_string())
                                .styled(Style::new().with_font_size(9).with_color(INK))
                                .padded(Margins::trbl(0, 0, 0, 12)),
                        );
                    }
                }
            }
        }

        if matches!(question.question_type, QuestionType::Code) {
            doc.push(elements::Break::new(0.2));
        }
        doc.push(elements::Break::new(0.7));
    }
}

fn labels_for(lang: &str) -> &'static Labels {
    if lang.eq_ignore_ascii_case("en") {
        &EN
    } else {
        &RU
    }
}

fn duration_label(test: &Test, labels: &Labels) -> String {
    if test.test_type.as_deref() == Some("presentation") {
        format!("{} {}", test.duration_minutes / 60, labels.hours)
    } else {
        format!("{} {}", test.duration_minutes, labels.minutes)
    }
}

fn total_duration(minutes: i32, labels: &Labels) -> String {
    let hours = minutes / 60;
    let rest = minutes % 60;
    match (hours, rest) {
        (0, _) => format!("{} {}", rest, labels.minutes),
        (_, 0) => format!("{} {}", hours, labels.hours),
        _ => format!("{} {} {} {}", hours, labels.hours, rest, labels.minutes),
    }
}

fn questions_of(test: &Test) -> Vec<Question> {
    serde_json::from_value(test.questions.clone()).unwrap_or_default()
}

fn themes_of(test: &Test) -> Vec<String> {
    test.presentation_themes
        .clone()
        .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
        .unwrap_or_default()
}

fn transliterate(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        if c.is_ascii() {
            out.push(c);
            continue;
        }
        let lower = c.to_lowercase().next().unwrap_or(c);
        let mapped = match lower {
            'а' => "a", 'б' => "b", 'в' => "v", 'г' => "g", 'д' => "d",
            'е' => "e", 'ё' => "e", 'ж' => "zh", 'з' => "z", 'и' => "i",
            'й' => "y", 'к' => "k", 'л' => "l", 'м' => "m", 'н' => "n",
            'о' => "o", 'п' => "p", 'р' => "r", 'с' => "s", 'т' => "t",
            'у' => "u", 'ф' => "f", 'х' => "kh", 'ц' => "ts", 'ч' => "ch",
            'ш' => "sh", 'щ' => "shch", 'ъ' => "", 'ы' => "y", 'ь' => "",
            'э' => "e", 'ю' => "yu", 'я' => "ya",
            _ => "",
        };
        if mapped.is_empty() {
            continue;
        }
        if c.is_uppercase() {
            let mut chars = mapped.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.push_str(chars.as_str());
            }
        } else {
            out.push_str(mapped);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_html(input: &str) -> String {
    let mut result = String::new();
    let mut inside_tag = false;

    for c in input.chars() {
        match c {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => result.push(c),
            _ => {}
        }
    }

    result
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_test(title: &str, presentation: bool) -> Test {
        let questions = if presentation {
            json!([])
        } else {
            json!([
                {
                    "id": 1,
                    "type": "multiple_choice",
                    "question": "Какой тип данных неизменяемый?",
                    "points": 2,
                    "options": ["list", "dict", "tuple"],
                    "correct_answer": 2,
                    "explanation": "Кортежи нельзя изменить после создания."
                },
                {
                    "id": 2,
                    "type": "short_answer",
                    "question": "Опишите разницу между list и generator",
                    "points": 3,
                    "expected_keywords": ["лениво", "память"],
                    "min_words": 20,
                    "ai_grading": true
                },
                {
                    "id": 3,
                    "type": "code",
                    "question": "Reverse a string",
                    "points": 5,
                    "language": "python",
                    "starter_code": "def reverse(s):\n    pass\n",
                    "test_cases": []
                }
            ])
        };

        serde_json::from_value(json!({
            "id": uuid::Uuid::new_v4(),
            "external_id": null,
            "title": title,
            "description": "Проверка знаний основ Python",
            "instructions": "Отвечайте самостоятельно",
            "questions": questions,
            "duration_minutes": if presentation { 120 } else { 45 },
            "passing_score": "70.00",
            "max_attempts": 2,
            "shuffle_questions": false,
            "shuffle_options": false,
            "show_results_immediately": true,
            "created_by": null,
            "is_active": !presentation,
            "test_type": if presentation { "presentation" } else { "question_based" },
            "presentation_themes": if presentation { json!(["Анализ рынка", "Roadmap"]) } else { json!(null) },
            "presentation_extra_info": if presentation { json!("15 минут на выступление") } else { json!(null) },
            "created_at": "2026-05-02T10:00:00Z",
            "updated_at": null
        }))
        .expect("sample test fixture should deserialize")
    }

    #[test]
    fn fixture_questions_parse() {
        assert_eq!(questions_of(&sample_test("T", false)).len(), 3);
        assert_eq!(themes_of(&sample_test("P", true)).len(), 2);
    }

    #[test]
    fn renders_single_test() {
        for lang in ["ru", "en"] {
            let pdf = PdfService::generate_test_pdf(&sample_test("Python", false), lang).unwrap();
            assert!(pdf.starts_with(b"%PDF"), "{lang}: not a pdf");
            assert!(pdf.len() > 2_000, "{lang}: suspiciously small pdf");
        }
    }

    #[test]
    fn renders_catalogue_of_every_test() {
        let tests = vec![
            sample_test("Python — базовый уровень", false),
            sample_test("Презентация: стратегия", true),
        ];

        for lang in ["ru", "en"] {
            let pdf = PdfService::generate_tests_catalogue_pdf(&tests, lang).unwrap();
            assert!(pdf.starts_with(b"%PDF"), "{lang}: not a pdf");
            assert!(pdf.len() > 5_000, "{lang}: suspiciously small catalogue");
        }
    }

    #[test]
    fn renders_empty_catalogue() {
        let pdf = PdfService::generate_tests_catalogue_pdf(&[], "ru").unwrap();
        assert!(pdf.starts_with(b"%PDF"));
    }
}
