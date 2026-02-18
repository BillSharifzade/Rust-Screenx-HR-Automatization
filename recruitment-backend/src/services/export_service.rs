use crate::models::candidate::{Candidate, HistoryItem};
use crate::error::Result;
use rust_xlsxwriter::*;
use std::collections::HashMap;
use uuid::Uuid;

pub struct ExportService;

impl ExportService {
    fn strip_html(input: &str) -> String {
        let mut result = String::new();
        let mut inside_tag = false;
        
        for c in input.chars() {
            if c == '<' {
                inside_tag = true;
            } else if c == '>' {
                inside_tag = false;
            } else if !inside_tag {
                result.push(c);
            }
        }
        
        result.trim().replace("&nbsp;", " ").replace("&quot;", "\"").replace("&amp;", "&").to_string()
    }
}

impl ExportService {
    /// Generate a styled XLSX workbook from a list of candidates.
    pub fn generate_candidates_xlsx(
        candidates: &[Candidate],
        vacancy_map: &HashMap<i64, String>,
        history_map: &HashMap<Uuid, Vec<HistoryItem>>
    ) -> Result<Vec<u8>> {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("Candidates")?;

        // ── Color palette ──
        let primary_color = Color::RGB(0x1E293B);     // Slate 800
        let header_bg = Color::RGB(0x0F172A);          // Slate 900
        let header_text = Color::White;
        let alt_row_1 = Color::RGB(0xF8FAFC);          // Slate 50
        let alt_row_2 = Color::White;
        let border_color = Color::RGB(0xE2E8F0);       // Slate 200
        // let accent_color = Color::RGB(0x6366F1);        // Indigo 500

        // Status colors
        let status_new = Color::RGB(0x3B82F6);          // Blue
        let status_reviewing = Color::RGB(0xF59E0B);    // Amber
        let status_contacted = Color::RGB(0x8B5CF6);    // Violet
        let status_accepted = Color::RGB(0x10B981);     // Emerald
        let status_rejected = Color::RGB(0xEF4444);     // Red

        // AI rating colors
        let rating_high = Color::RGB(0x10B981);        // Emerald (70+)
        let rating_mid = Color::RGB(0xF59E0B);         // Amber (40-69)
        let rating_low = Color::RGB(0xEF4444);         // Red (<40)

        // ── Column definitions ──
        let columns = [
            ("№",                8.0),
            ("ФИО",              30.0),
            ("Email",            30.0),
            ("Телефон",          18.0),
            ("Дата рождения",    16.0),
            ("Telegram ID",      14.0),
            ("Статус",           16.0),
            ("AI Рейтинг (%)",   16.0),
            ("AI Комментарий",   50.0),
            ("Вакансия",         35.0),
            ("История активности", 60.0),
            ("Дата регистрации", 20.0),
            ("Последнее обновление", 22.0),
            ("Непрочит. сообщ.", 16.0),
        ];

        // Set column widths
        for (i, (_, width)) in columns.iter().enumerate() {
            worksheet.set_column_width(i as u16, *width)?;
        }

        // ── Title row ──
        let title_format = Format::new()
            .set_font_size(16)
            .set_bold()
            .set_font_color(header_text)
            .set_background_color(primary_color)
            .set_align(FormatAlign::CenterAcross)
            .set_align(FormatAlign::VerticalCenter);

        worksheet.set_row_height(0, 40)?;
        worksheet.merge_range(0, 0, 0, (columns.len() - 1) as u16, "Отчёт по кандидатам", &title_format)?;

        // ── Subtitle row ──
        let subtitle_format = Format::new()
            .set_font_size(10)
            .set_italic()
            .set_font_color(Color::RGB(0x94A3B8))
            .set_background_color(primary_color)
            .set_align(FormatAlign::CenterAcross)
            .set_align(FormatAlign::VerticalCenter);

        worksheet.set_row_height(1, 22)?;
        let now = chrono::Utc::now().format("%d.%m.%Y %H:%M UTC").to_string();
        let subtitle_text = format!("Дата экспорта: {}  •  Всего кандидатов: {}", now, candidates.len());
        worksheet.merge_range(1, 0, 1, (columns.len() - 1) as u16, &subtitle_text, &subtitle_format)?;

        // ── Header row ──
        let header_format = Format::new()
            .set_bold()
            .set_font_size(10)
            .set_font_color(header_text)
            .set_background_color(header_bg)
            .set_align(FormatAlign::Center)
            .set_align(FormatAlign::VerticalCenter)
            .set_text_wrap()
            .set_border(FormatBorder::Thin)
            .set_border_color(border_color);

        let header_row = 2;
        worksheet.set_row_height(header_row, 30)?;
        for (i, (name, _)) in columns.iter().enumerate() {
            worksheet.write_string_with_format(header_row, i as u16, *name, &header_format)?;
        }

        // ── Data rows ──
        let data_start_row = 3;
        for (idx, candidate) in candidates.iter().enumerate() {
            let row = data_start_row + idx as u32;
            let bg = if idx % 2 == 0 { alt_row_1 } else { alt_row_2 };

            let base_fmt = Format::new()
                .set_font_size(10)
                .set_background_color(bg)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin)
                .set_border_color(border_color);

            let center_fmt = base_fmt.clone().set_align(FormatAlign::Center);
            let wrap_fmt = base_fmt.clone().set_text_wrap();

            worksheet.set_row_height(row, 22)?;

            // № (row number)
            worksheet.write_number_with_format(row, 0, (idx + 1) as f64, &center_fmt)?;

            // ФИО
            let name_fmt = base_fmt.clone().set_bold();
            worksheet.write_string_with_format(row, 1, &candidate.name, &name_fmt)?;

            // Email
            worksheet.write_string_with_format(row, 2, &candidate.email, &base_fmt)?;

            // Phone
            worksheet.write_string_with_format(row, 3, candidate.phone.as_deref().unwrap_or("—"), &base_fmt)?;

            // DOB
            let dob_str = candidate.dob
                .map(|d| d.format("%d.%m.%Y").to_string())
                .unwrap_or_else(|| "—".to_string());
            worksheet.write_string_with_format(row, 4, &dob_str, &center_fmt)?;

            // Telegram ID
            let tg_str = candidate.telegram_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "—".to_string());
            worksheet.write_string_with_format(row, 5, &tg_str, &center_fmt)?;

            // Status (colored)
            let status_color = match candidate.status.as_str() {
                "new" => status_new,
                "reviewing" => status_reviewing,
                "contacted" => status_contacted,
                "accepted" => status_accepted,
                "rejected" => status_rejected,
                _ => Color::RGB(0x64748B),
            };
            let status_display = match candidate.status.as_str() {
                "new" => "Новый",
                "reviewing" => "Рассмотрение",
                "contacted" => "Связались",
                "accepted" => "Приняты",
                "rejected" => "Отказано",
                _ => &candidate.status,
            };
            let status_fmt = Format::new()
                .set_font_size(10)
                .set_bold()
                .set_font_color(Color::White)
                .set_background_color(status_color)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin)
                .set_border_color(border_color);
            worksheet.write_string_with_format(row, 6, status_display, &status_fmt)?;

            // AI Rating (color-coded)
            if let Some(rating) = candidate.ai_rating {
                let r_color = if rating >= 70 { rating_high } else if rating >= 40 { rating_mid } else { rating_low };
                let rating_fmt = Format::new()
                    .set_font_size(11)
                    .set_bold()
                    .set_font_color(r_color)
                    .set_background_color(bg)
                    .set_align(FormatAlign::Center)
                    .set_align(FormatAlign::VerticalCenter)
                    .set_border(FormatBorder::Thin)
                    .set_border_color(border_color);
                worksheet.write_number_with_format(row, 7, rating as f64, &rating_fmt)?;
            } else {
                worksheet.write_string_with_format(row, 7, "—", &center_fmt)?;
            }

            // AI Comment
            let comment = candidate.ai_comment.as_deref().unwrap_or("—");
            worksheet.write_string_with_format(row, 8, comment, &wrap_fmt)?;

            // Vacancy
            let vac_name = candidate.vacancy_id
                .and_then(|id| vacancy_map.get(&id))
                .map(|s| Self::strip_html(s))
                .unwrap_or_else(|| "—".to_string());
            let vac_display = if let Some(id) = candidate.vacancy_id {
                format!("{} (id:{})", vac_name, id)
            } else {
                "—".to_string()
            };
            worksheet.write_string_with_format(row, 9, &vac_display, &wrap_fmt)?;

            // Interaction Story (History)
            let mut story = String::new();
            if let Some(hist) = history_map.get(&candidate.id) {
                for (h_idx, item) in hist.iter().enumerate() {
                    let date = item.timestamp.with_timezone(&chrono::Local).format("%d.%m").to_string();
                    let title = match item.event_type.as_str() {
                        "registration" => "Регистрация",
                        "application" => "Отклик",
                        "profile_update" => "Обновление",
                        "test_attempt" => "Тест",
                        _ => &item.event_type,
                    };
                    let status = if let Some(s) = &item.status {
                        let translated = match s.as_str() {
                            "candidate_profile.status_completed" => "Завершено",
                            "candidate_profile.status_passed" => "Пройден",
                            "candidate_profile.status_failed" => "Не пройден",
                            "candidate_profile.status_submitted" => "Отправлено",
                            "dashboard.invites.statuses.pending" => "Ожидает",
                            "dashboard.invites.statuses.in_progress" => "В процессе",
                            "dashboard.invites.statuses.timeout" => "Время вышло",
                            "dashboard.invites.statuses.escaped" => "Покинул",
                            "dashboard.invites.statuses.needs_review" => "Проверка",
                            _ => s,
                        };
                        format!(" [{}]", translated)
                    } else {
                        "".to_string()
                    };
                    story.push_str(&format!("{}. {}: {}{}", date, title, item.description.as_deref().unwrap_or("—"), status));
                    if h_idx < hist.len() - 1 && h_idx < 5 { // Limit to 5 items to keep cell readable
                        story.push('\n');
                    }
                    if h_idx == 5 {
                        story.push_str("\n...");
                        break;
                    }
                }
            }
            if story.is_empty() { story = "—".to_string(); }
            worksheet.write_string_with_format(row, 10, &story, &wrap_fmt)?;

            // Created At
            let created_str = candidate.created_at
                .map(|d| d.format("%d.%m.%Y %H:%M").to_string())
                .unwrap_or_else(|| "—".to_string());
            worksheet.write_string_with_format(row, 11, &created_str, &center_fmt)?;

            // Updated At
            let updated_str = candidate.updated_at
                .map(|d| d.format("%d.%m.%Y %H:%M").to_string())
                .unwrap_or_else(|| "—".to_string());
            worksheet.write_string_with_format(row, 12, &updated_str, &center_fmt)?;

            // Unread messages
            let unread = candidate.unread_messages.unwrap_or(0);
            if unread > 0 {
                let unread_fmt = Format::new()
                    .set_font_size(10)
                    .set_bold()
                    .set_font_color(Color::White)
                    .set_background_color(status_new)
                    .set_align(FormatAlign::Center)
                    .set_align(FormatAlign::VerticalCenter)
                    .set_border(FormatBorder::Thin)
                    .set_border_color(border_color);
                worksheet.write_number_with_format(row, 13, unread as f64, &unread_fmt)?;
            } else {
                worksheet.write_string_with_format(row, 13, "0", &center_fmt)?;
            }
        }

        // ── Summary row ──
        let total_row = data_start_row + candidates.len() as u32 + 1;
        let summary_fmt = Format::new()
            .set_bold()
            .set_font_size(10)
            .set_font_color(primary_color)
            .set_background_color(Color::RGB(0xE0E7FF))  // Indigo 100
            .set_align(FormatAlign::Center)
            .set_align(FormatAlign::VerticalCenter)
            .set_border(FormatBorder::Thin)
            .set_border_color(border_color);

        worksheet.set_row_height(total_row, 26)?;
        worksheet.merge_range(total_row, 0, total_row, 1, &format!("Итого: {} кандидатов", candidates.len()), &summary_fmt)?;

        // Status counts
        let new_count = candidates.iter().filter(|c| c.status == "new").count();
        let reviewing_count = candidates.iter().filter(|c| c.status == "reviewing").count();
        let contacted_count = candidates.iter().filter(|c| c.status == "contacted").count();
        let accepted_count = candidates.iter().filter(|c| c.status == "accepted").count();
        let rejected_count = candidates.iter().filter(|c| c.status == "rejected").count();

        let status_summary = format!(
            "Новые: {} | Рассмотрение: {} | Связались: {} | Приняты: {} | Отказано: {}",
            new_count, reviewing_count, contacted_count, accepted_count, rejected_count
        );
        worksheet.merge_range(total_row, 2, total_row, 5, &status_summary, &summary_fmt)?;

        // Average AI rating
        let ratings: Vec<i32> = candidates.iter().filter_map(|c| c.ai_rating).collect();
        let avg_rating = if ratings.is_empty() {
            0.0
        } else {
            ratings.iter().sum::<i32>() as f64 / ratings.len() as f64
        };
        let top_talents = candidates.iter().filter(|c| c.ai_rating.unwrap_or(0) >= 70).count();
        let highly_engaged = candidates.iter().filter(|c| history_map.get(&c.id).map(|h| h.len()).unwrap_or(0) >= 3).count();

        let stats_summary = format!(
            "Ср. рейтинг: {:.0}% | Топ-таланты (70%+): {} | Активные (3+ действия): {}",
            avg_rating, top_talents, highly_engaged
        );
        worksheet.merge_range(total_row, 6, total_row, 10, &stats_summary, &summary_fmt)?;

        // Fill remaining summary cells
        for col in 8..columns.len() as u16 {
            worksheet.write_string_with_format(total_row, col, "", &summary_fmt)?;
        }

        // Freeze panes (header stays visible while scrolling)
        worksheet.set_freeze_panes(3, 0)?;

        // Auto-filter on data columns
        worksheet.autofilter(2, 0, (data_start_row + candidates.len() as u32 - 1).max(2), (columns.len() - 1) as u16)?;

        let buffer = workbook.save_to_buffer()?;
        Ok(buffer)
    }
}
