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
};

const EN: Labels = Labels {
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
};

impl PdfService {
    pub fn generate_test_pdf(test: &Test, lang: &str) -> Result<Vec<u8>> {
        let labels = if lang.eq_ignore_ascii_case("en") { &EN } else { &RU };

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
        doc.set_title(transliterate(&strip_html(&test.title)));
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

        Self::push_header(&mut doc, test, labels);

        let is_presentation = test.test_type.as_deref() == Some("presentation");
        if is_presentation {
            Self::push_presentation_body(&mut doc, test, labels);
        } else {
            Self::push_questions(&mut doc, test, labels);
        }

        let mut buffer = Vec::new();
        doc.render(&mut buffer)
            .map_err(|e| Error::Internal(format!("pdf render failed: {}", e)))?;
        Ok(buffer)
    }

    fn push_header(doc: &mut genpdf::Document, test: &Test, labels: &Labels) {
        doc.push(
            elements::Paragraph::new(strip_html(&test.title))
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
        let duration = if is_presentation {
            format!("{} {}", test.duration_minutes / 60, labels.hours)
        } else {
            format!("{} {}", test.duration_minutes, labels.minutes)
        };

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
