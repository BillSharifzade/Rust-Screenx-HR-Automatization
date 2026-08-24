use crate::error::{Error, Result};
use crate::models::interview_form::*;
use crate::services::pdf_service::{transliterate, PdfService};
use genpdf::style::{Color, Style};
use genpdf::{elements, Alignment, Element, Margins, SimplePageDecorator};

const INK: Color = Color::Rgb(17, 24, 39);
const MUTED: Color = Color::Rgb(107, 114, 128);
const FLAG: Color = Color::Rgb(180, 83, 9);

const TITLE_1: &str =
    "Форма оценки соискателя в ходе проведения 1-го личного собеседования";
const TITLE_2: &str =
    "Форма оценки соискателя в ходе проведения 2-го личного собеседования";

fn label_style() -> Style {
    Style::new().with_font_size(9).with_color(MUTED)
}

fn value_style() -> Style {
    Style::new().with_font_size(10).with_color(INK)
}

fn header_style() -> Style {
    Style::new().bold().with_font_size(8).with_color(MUTED)
}

fn section_style() -> Style {
    Style::new().bold().with_font_size(10).with_color(INK)
}

fn cell_padding() -> Margins {
    Margins::trbl(1.5, 2.0, 1.5, 2.0)
}

fn text_block(value: &str, style: Style) -> elements::LinearLayout {
    let mut layout = elements::LinearLayout::vertical();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        layout.push(elements::Paragraph::new(" ").styled(style));
        return layout;
    }
    for line in trimmed.lines() {
        layout.push(elements::Paragraph::new(line.trim_end()).styled(style));
    }
    layout
}

fn opt(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("")
}

fn table(weights: Vec<usize>) -> elements::TableLayout {
    let mut table = elements::TableLayout::new(weights);
    table.set_cell_decorator(elements::FrameCellDecorator::new(true, true, true));
    table
}

fn push_row(
    table: &mut elements::TableLayout,
    left: &str,
    left_style: Style,
    right: &str,
    right_style: Style,
) -> Result<()> {
    table
        .row()
        .element(text_block(left, left_style).padded(cell_padding()))
        .element(text_block(right, right_style).padded(cell_padding()))
        .push()
        .map_err(|e| Error::Internal(format!("pdf table failed: {}", e)))
}

fn section(doc: &mut genpdf::Document, title: &str) {
    doc.push(elements::Break::new(0.7));
    doc.push(elements::Paragraph::new(title).styled(section_style()));
    doc.push(elements::Break::new(0.25));
}

fn free_text(doc: &mut genpdf::Document, title: &str, body: &Option<String>) {
    let Some(text) = body.as_deref().map(str::trim).filter(|t| !t.is_empty()) else {
        return;
    };
    section(doc, title);
    let mut framed = table(vec![1]);
    framed
        .row()
        .element(text_block(text, value_style()).padded(cell_padding()))
        .push()
        .ok();
    doc.push(framed);
}

pub struct InterviewFormPdf;

impl InterviewFormPdf {
    pub fn render(recognition: &InterviewFormRecognition) -> Result<Vec<u8>> {
        let source = recognition
            .corrected_fields
            .clone()
            .unwrap_or_else(|| recognition.fields.clone());
        let form: RecognizedForm = serde_json::from_value(source)
            .map_err(|e| Error::Internal(format!("stored form is not renderable: {}", e)))?;

        let title = match form.form_type.as_str() {
            FORM_TYPE_INTERVIEW_1 => TITLE_1,
            FORM_TYPE_INTERVIEW_2 => TITLE_2,
            _ => "Форма оценки соискателя",
        };

        let mut doc = genpdf::Document::new(PdfService::font_family()?);
        doc.set_title(transliterate(title));
        doc.set_minimal_conformance();
        doc.set_font_size(10);
        doc.set_line_spacing(1.3);

        let mut decorator = SimplePageDecorator::new();
        decorator.set_margins(Margins::trbl(16, 16, 16, 16));
        decorator.set_header(|page| {
            let mut layout = elements::LinearLayout::vertical();
            if page > 1 {
                layout.push(
                    elements::Paragraph::new(format!("стр. {}", page))
                        .aligned(Alignment::Right)
                        .styled(Style::new().with_font_size(8).with_color(MUTED)),
                );
                layout.push(elements::Break::new(0.5));
            }
            layout
        });
        doc.set_page_decorator(decorator);

        doc.push(
            elements::Paragraph::new("ГК «КОИНОТИ НАВ»")
                .aligned(Alignment::Center)
                .styled(Style::new().with_font_size(9).with_color(MUTED)),
        );
        doc.push(elements::Break::new(0.3));
        doc.push(
            elements::Paragraph::new(title)
                .aligned(Alignment::Center)
                .styled(Style::new().bold().with_font_size(12).with_color(INK)),
        );
        doc.push(elements::Break::new(0.8));

        Self::push_header(&mut doc, &form)?;
        Self::push_parameters(&mut doc, &form)?;

        if form.form_type == FORM_TYPE_INTERVIEW_2 {
            Self::push_decisions(&mut doc, &form)?;
        } else {
            Self::push_grid(&mut doc, "1-3 сильных качества соискателя", &form.strengths)?;
            Self::push_grid(&mut doc, "1-3 точки роста соискателя", &form.growth_areas)?;
        }

        free_text(
            &mut doc,
            "Комментарии и общие впечатления о соискателе",
            &form.comments,
        );
        free_text(&mut doc, "Выводы", &form.conclusions);
        free_text(&mut doc, "Рекомендации ДЧР", &form.hr_department_recommendation);
        free_text(&mut doc, "Итоги тестов БЮД", &form.test_results_bud);
        free_text(&mut doc, "Итоги тестов ФЭД", &form.test_results_fed);

        if !form.extra_notes.is_empty() {
            section(&mut doc, "Прочие пометки");
            for note in &form.extra_notes {
                doc.push(elements::Paragraph::new(format!("• {}", note)).styled(value_style()));
            }
        }

        Self::push_footer(&mut doc, recognition);

        let mut buffer = Vec::new();
        doc.render(&mut buffer)
            .map_err(|e| Error::Internal(format!("pdf render failed: {}", e)))?;
        Ok(buffer)
    }

    fn push_header(doc: &mut genpdf::Document, form: &RecognizedForm) -> Result<()> {
        let mut head = table(vec![4, 6]);
        let label = label_style();
        let value = value_style();

        push_row(&mut head, "Интервьюер", label, &form.interviewers.join(", "), value)?;
        if form.form_type == FORM_TYPE_INTERVIEW_2 {
            push_row(&mut head, "Должность", label, opt(&form.interviewer_position), value)?;
            push_row(&mut head, "Департамент", label, opt(&form.department), value)?;
            push_row(&mut head, "Отдел", label, opt(&form.division), value)?;
        }
        push_row(&mut head, "Дата интервью", label, opt(&form.interview_date), value)?;
        push_row(&mut head, "Ф.И.О. соискателя", label, opt(&form.candidate_name), value)?;
        if let Some(age) = form.candidate_age {
            push_row(&mut head, "Возраст", label, &age.to_string(), value)?;
        }
        push_row(
            &mut head,
            "Обсуждаемая должность",
            label,
            opt(&form.position_discussed),
            value,
        )?;
        push_row(
            &mut head,
            "Установленное время начала интервью",
            label,
            opt(&form.scheduled_start_time),
            value,
        )?;
        push_row(
            &mut head,
            "Фактическое время прихода соискателя",
            label,
            opt(&form.actual_arrival_time),
            value,
        )?;

        let duration = match (form.interview_from.as_deref(), form.interview_to.as_deref()) {
            (None, None) => String::new(),
            (from, to) => format!("с {} до {}", from.unwrap_or("___"), to.unwrap_or("___")),
        };
        push_row(&mut head, "Продолжительность собеседования", label, &duration, value)?;

        doc.push(head);
        Ok(())
    }

    fn push_parameters(doc: &mut genpdf::Document, form: &RecognizedForm) -> Result<()> {
        if form.parameters.is_empty() {
            return Ok(());
        }
        section(doc, "Параметры");

        let mut params = table(vec![4, 6]);
        params
            .row()
            .element(text_block("ПАРАМЕТРЫ", header_style()).padded(cell_padding()))
            .element(text_block("ИНФОРМАЦИЯ", header_style()).padded(cell_padding()))
            .push()
            .map_err(|e| Error::Internal(format!("pdf table failed: {}", e)))?;

        for row in &form.parameters {
            push_row(
                &mut params,
                &row.label,
                label_style(),
                opt(&row.value),
                value_style(),
            )?;
        }

        doc.push(params);
        Ok(())
    }

    fn push_grid(doc: &mut genpdf::Document, title: &str, rows: &[SkillRow]) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        section(doc, title);

        let mut grid = table(vec![1, 5, 5]);
        grid.row()
            .element(text_block("№", header_style()).padded(cell_padding()))
            .element(text_block("PROF & SOFT SKILLS", header_style()).padded(cell_padding()))
            .element(text_block("ЛИЧНЫЕ КАЧЕСТВА", header_style()).padded(cell_padding()))
            .push()
            .map_err(|e| Error::Internal(format!("pdf table failed: {}", e)))?;

        for row in rows {
            grid.row()
                .element(text_block(&row.index.to_string(), value_style()).padded(cell_padding()))
                .element(
                    text_block(opt(&row.prof_soft_skills), value_style()).padded(cell_padding()),
                )
                .element(
                    text_block(opt(&row.personal_qualities), value_style()).padded(cell_padding()),
                )
                .push()
                .map_err(|e| Error::Internal(format!("pdf table failed: {}", e)))?;
        }

        doc.push(grid);
        Ok(())
    }

    fn push_decisions(doc: &mut genpdf::Document, form: &RecognizedForm) -> Result<()> {
        for (title, block) in [
            (
                "Комментарии и решение со стороны Инициатора заявки",
                &form.requester_decision,
            ),
            (
                "Комментарии и решение со стороны ДЧР",
                &form.hr_decision,
            ),
        ] {
            let Some(block) = block else { continue };
            section(doc, title);

            let mut decision = table(vec![4, 6]);
            push_row(&mut decision, "Ф.И.О.", label_style(), opt(&block.full_name), value_style())?;
            push_row(&mut decision, "Должность", label_style(), opt(&block.position), value_style())?;
            push_row(
                &mut decision,
                "Комментарий",
                label_style(),
                opt(&block.comment),
                value_style(),
            )?;
            doc.push(decision);
        }
        Ok(())
    }

    fn push_footer(doc: &mut genpdf::Document, recognition: &InterviewFormRecognition) {
        doc.push(elements::Break::new(0.8));

        let mut line = format!(
            "Цифровая расшифровка рукописного бланка · {} · {}",
            recognition.id,
            recognition.created_at.format("%Y-%m-%d %H:%M UTC")
        );
        if let Some(confidence) = recognition.overall_confidence {
            line.push_str(&format!(" · уверенность {:.0}%", confidence * 100.0));
        }
        doc.push(
            elements::Paragraph::new(line)
                .styled(Style::new().with_font_size(7).with_color(MUTED)),
        );

        let low: Vec<String> = serde_json::from_value(recognition.low_confidence_fields.clone())
            .unwrap_or_default();
        if recognition.needs_review {
            let mut note = String::from("Требуется проверка человеком");
            if !low.is_empty() {
                note.push_str(&format!(": {}", low.join(", ")));
            }
            doc.push(
                elements::Paragraph::new(note)
                    .styled(Style::new().with_font_size(7).with_color(FLAG)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn recognition(fields: serde_json::Value) -> InterviewFormRecognition {
        InterviewFormRecognition {
            id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            candidate_id: None,
            source_url: "http://example/scan.jpg".into(),
            source_index: 0,
            image_sha256: None,
            form_type: fields
                .get("form_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            fields,
            field_confidence: serde_json::json!({}),
            overall_confidence: Some(0.91),
            needs_review: true,
            low_confidence_fields: serde_json::json!(["candidate_name"]),
            warnings: serde_json::json!([]),
            raw_model_output: None,
            corrected_fields: None,
            reviewed_by: None,
            reviewed_at: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn renders_a_first_interview_sheet() {
        let pdf = InterviewFormPdf::render(&recognition(serde_json::json!({
            "form_type": "interview_1",
            "interviewers": ["Шарипова М.", "Гулов А."],
            "interview_date": "2026-08-12",
            "candidate_name": "Шарипов Исмон",
            "position_discussed": "Бухгалтер",
            "scheduled_start_time": "10:00",
            "actual_arrival_time": "10:05",
            "interview_from": "10:05",
            "interview_to": "11:10",
            "parameters": [
                {"key": "vacancy_source", "label": "Откуда узнали о вакансии", "value": "Сайт"},
                {"key": "salary_expectation", "label": "ЗП (минимальный/ожидаемый)", "value": "5000/6000"}
            ],
            "strengths": [{"index": 1, "prof_soft_skills": "1С, Excel", "personal_qualities": "Ответственный"}],
            "growth_areas": [{"index": 1, "prof_soft_skills": "Английский", "personal_qualities": null}],
            "comments": "1. Опыт 5 лет\n2. Готов выйти сразу",
            "conclusions": "Рекомендую",
            "hr_department_recommendation": "Пригласить на 2-й этап"
        })))
        .expect("render");

        assert!(pdf.starts_with(b"%PDF"), "not a pdf");
        assert!(pdf.len() > 2_000, "suspiciously small: {} bytes", pdf.len());
    }

    #[test]
    fn renders_a_second_interview_sheet_with_decision_blocks() {
        let pdf = InterviewFormPdf::render(&recognition(serde_json::json!({
            "form_type": "interview_2",
            "interviewers": ["Каримов Р."],
            "interviewer_position": "Начальник отдела",
            "department": "ДЧР",
            "division": "Подбор",
            "candidate_name": "Ходжаев Адам",
            "parameters": [{"key": null, "label": "Причина переезда", "value": "Семья"}],
            "requester_decision": {"full_name": "Иванов И.", "position": "Директор", "comment": "Согласовано"},
            "hr_decision": {"full_name": "Петрова А.", "position": "HR", "comment": "Оформляем"}
        })))
        .expect("render");

        assert!(pdf.starts_with(b"%PDF"));
    }

    #[test]
    fn an_almost_empty_sheet_still_renders() {
        let pdf = InterviewFormPdf::render(&recognition(serde_json::json!({
            "form_type": "unknown"
        })))
        .expect("render");

        assert!(pdf.starts_with(b"%PDF"));
    }
}
