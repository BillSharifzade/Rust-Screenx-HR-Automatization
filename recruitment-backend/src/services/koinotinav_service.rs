//! Client for the public Koinotinav job board (job.koinotinav.tj).
//!
//! The board used to serve `/api/vacancies` and `/api/companies` as JSON. Those
//! routes disappeared when the site was rebuilt, so the vacancy list is now read
//! from its server-rendered HTML pages. The JSON API is still tried first in
//! case it comes back.
//!
//! Callers (dashboard, candidate registration, OneF endpoints, AI scoring) all
//! read one in-memory snapshot that a background task keeps warm. When the board
//! is unreachable the last good snapshot keeps being served instead of failing
//! every page that lists vacancies.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use reqwest::{header, Client, StatusCode};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Deserializer, Serialize};
use tokio::sync::{Mutex, RwLock, Semaphore};

use crate::error::{Error, Result};

const DEFAULT_BASE_URL: &str = "https://job.koinotinav.tj";
/// How long a snapshot is served before it is rebuilt.
const SNAPSHOT_TTL: Duration = Duration::from_secs(10 * 60);
/// Vacancy descriptions rarely change, so each detail page is refetched less often.
const DETAIL_TTL: Duration = Duration::from_secs(6 * 60 * 60);
/// Guard against following a broken pagination forever.
const MAX_PAGES: u32 = 50;
const MAX_CONCURRENT_REQUESTS: usize = 4;

fn strip_html_tags(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    for c in input.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            output.push(c);
        }
    }
    output.trim().to_string()
}

fn deserialize_bool_flexible<'de, D>(deserializer: D) -> std::result::Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrInt {
        Bool(bool),
        Int(i64),
        String(String),
    }

    match BoolOrInt::deserialize(deserializer)? {
        BoolOrInt::Bool(b) => Ok(b),
        BoolOrInt::Int(i) => Ok(i != 0),
        BoolOrInt::String(s) => match s.as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(serde::de::Error::custom(format!("Invalid boolean string: {}", s))),
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalVacancy {
    pub id: i64,
    pub title: String,
    pub content: String,
    #[serde(deserialize_with = "deserialize_bool_flexible")]
    pub hot: bool,
    pub city: String,
    pub direction: String,
    pub company_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalCompany {
    pub id: i64,
    pub title: String,
    pub logo: String,
}

#[derive(Clone)]
pub struct KoinotinavService {
    client: Client,
    base_url: Arc<str>,
    cache: Arc<RwLock<Cache>>,
    refresh_lock: Arc<Mutex<()>>,
}

#[derive(Default)]
struct Cache {
    snapshot: Option<Arc<Snapshot>>,
    details: HashMap<i64, CachedDetail>,
    /// The HTML pages carry no dates, so a scraped vacancy is dated by when it
    /// was first seen. The frontend formats `created_at` and cannot take "".
    first_seen: HashMap<i64, DateTime<Utc>>,
}

struct Snapshot {
    vacancies: Vec<ExternalVacancy>,
    companies: Vec<ExternalCompany>,
    fetched_at: Instant,
}

struct CachedDetail {
    content: String,
    fetched_at: Instant,
}

impl Default for KoinotinavService {
    fn default() -> Self {
        Self::new()
    }
}

impl KoinotinavService {
    pub fn new() -> Self {
        let base_url = std::env::var("KOINOTINAV_BASE_URL")
            .ok()
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(20))
            .user_agent(concat!("ScreenX-HR/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("failed to build job board HTTP client");

        Self {
            client,
            base_url: base_url.into(),
            cache: Arc::new(RwLock::new(Cache::default())),
            refresh_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Rebuilds the snapshot every [`SNAPSHOT_TTL`] so requests never wait on the board.
    pub fn spawn_background_refresh(&self) {
        let this = self.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = this.refresh(true).await {
                    tracing::warn!("Job board refresh failed: {}", e);
                }
                tokio::time::sleep(SNAPSHOT_TTL).await;
            }
        });
    }

    pub async fn fetch_vacancies(&self) -> Result<Vec<ExternalVacancy>> {
        Ok(self.snapshot().await?.vacancies.clone())
    }

    pub async fn fetch_companies(&self) -> Result<Vec<ExternalCompany>> {
        Ok(self.snapshot().await?.companies.clone())
    }

    pub async fn fetch_vacancy(&self, id: i64) -> Result<Option<ExternalVacancy>> {
        match self.snapshot().await {
            Ok(snapshot) => {
                if let Some(v) = snapshot.vacancies.iter().find(|v| v.id == id) {
                    return Ok(Some(v.clone()));
                }
            }
            Err(e) => tracing::warn!(
                "Job board list unavailable, looking up vacancy {} directly: {}",
                id,
                e
            ),
        }
        // Not on the public list (unpublished, or the list is unreachable);
        // its own page may still exist.
        self.fetch_detail_page(id).await
    }

    async fn snapshot(&self) -> Result<Arc<Snapshot>> {
        if let Some(fresh) = self.cached_snapshot(true).await {
            return Ok(fresh);
        }
        match self.refresh(false).await {
            Ok(snapshot) => Ok(snapshot),
            Err(e) => match self.cached_snapshot(false).await {
                Some(stale) => {
                    tracing::warn!(
                        "Job board unreachable, serving snapshot from {}s ago: {}",
                        stale.fetched_at.elapsed().as_secs(),
                        e
                    );
                    Ok(stale)
                }
                None => Err(e),
            },
        }
    }

    async fn cached_snapshot(&self, fresh_only: bool) -> Option<Arc<Snapshot>> {
        let cache = self.cache.read().await;
        cache
            .snapshot
            .as_ref()
            .filter(|s| !fresh_only || s.fetched_at.elapsed() < SNAPSHOT_TTL)
            .cloned()
    }

    /// Concurrent callers share a single rebuild.
    async fn refresh(&self, force: bool) -> Result<Arc<Snapshot>> {
        let _guard = self.refresh_lock.lock().await;
        if !force {
            if let Some(fresh) = self.cached_snapshot(true).await {
                return Ok(fresh);
            }
        }

        let started = Instant::now();
        let (mut vacancies, companies, source) = match self.fetch_legacy_api().await {
            Some((vacancies, companies)) => (vacancies, companies, "json api"),
            None => {
                let (vacancies, companies) = self.scrape_site().await?;
                (vacancies, companies, "html")
            }
        };

        let mut cache = self.cache.write().await;
        let now = Utc::now();
        for v in &mut vacancies {
            if DateTime::parse_from_rfc3339(&v.created_at).is_err() {
                v.created_at = cache.first_seen.entry(v.id).or_insert(now).to_rfc3339();
            }
        }
        tracing::info!(
            "Job board snapshot: {} vacancies, {} companies (via {}, {} ms)",
            vacancies.len(),
            companies.len(),
            source,
            started.elapsed().as_millis()
        );
        let snapshot = Arc::new(Snapshot {
            vacancies,
            companies,
            fetched_at: Instant::now(),
        });
        cache.snapshot = Some(snapshot.clone());
        Ok(snapshot)
    }

    async fn fetch_legacy_api(&self) -> Option<(Vec<ExternalVacancy>, Vec<ExternalCompany>)> {
        let vacancies: Vec<ExternalVacancy> = self.get_json("/api/vacancies").await?;
        let companies: Vec<ExternalCompany> =
            self.get_json("/api/companies").await.unwrap_or_default();
        let vacancies = vacancies
            .into_iter()
            .map(|mut v| {
                v.title = strip_html_tags(&v.title);
                v.content = ammonia::clean(&v.content);
                v
            })
            .collect();
        Some((vacancies, companies))
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Option<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .header(header::ACCEPT, "application/json")
            .send()
            .await
            .ok()?;
        let is_json = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.contains("json"));
        if !resp.status().is_success() || !is_json {
            tracing::debug!("Job board JSON API {} unavailable ({})", path, resp.status());
            return None;
        }
        resp.json().await.ok()
    }

    async fn scrape_site(&self) -> Result<(Vec<ExternalVacancy>, Vec<ExternalCompany>)> {
        let first_html = self.get_html("/vacancies?page=1").await?;
        let first = parse_list_page(&first_html).ok_or_else(|| {
            Error::BadGateway("Job board vacancy list has an unexpected layout".into())
        })?;

        // A missing list page would silently drop vacancies, so any failure here
        // fails the refresh and the previous snapshot stays in use.
        let mut items = first.items.clone();
        let rest: Vec<String> = (2..=first.last_page.min(MAX_PAGES))
            .map(|p| format!("/vacancies?page={}", p))
            .collect();
        for (path, page) in self.get_many(rest).await {
            let html = page?.ok_or_else(|| Error::BadGateway(format!("{} not found", path)))?;
            let parsed = parse_list_page(&html)
                .ok_or_else(|| Error::BadGateway(format!("{} has an unexpected layout", path)))?;
            items.extend(parsed.items);
        }
        let mut seen = std::collections::HashSet::new();
        items.retain(|item| seen.insert(item.id));

        // Company and category are only visible through the list filters.
        let company_ids: Vec<i64> = first.companies.iter().map(|(id, _)| *id).collect();
        let category_ids: Vec<i64> = first.categories.iter().map(|(id, _)| *id).collect();
        let by_company = self.collect_filtered("company", &company_ids).await;
        let by_category = self.collect_filtered("category", &category_ids).await;

        let mut company_of: HashMap<i64, i64> = HashMap::new();
        let mut logos: HashMap<i64, String> = HashMap::new();
        for (company_id, listed) in &by_company {
            for item in listed {
                company_of.entry(item.id).or_insert(*company_id);
            }
            if let Some(logo) = listed.iter().map(|i| &i.logo).find(|l| !l.is_empty()) {
                logos.insert(*company_id, logo.clone());
            }
        }
        let category_names: HashMap<i64, &str> = first
            .categories
            .iter()
            .map(|(id, name)| (*id, name.as_str()))
            .collect();
        let mut direction_of: HashMap<i64, String> = HashMap::new();
        for (category_id, listed) in &by_category {
            let name = category_names.get(category_id).copied().unwrap_or_default();
            for item in listed {
                direction_of.entry(item.id).or_insert_with(|| name.to_string());
            }
        }

        let contents = self
            .vacancy_contents(&items.iter().map(|i| i.id).collect::<Vec<_>>())
            .await;

        let vacancies = items
            .into_iter()
            .map(|item| ExternalVacancy {
                id: item.id,
                content: contents.get(&item.id).cloned().unwrap_or_else(|| {
                    format!("<p>{}</p>", ammonia::clean_text(&item.summary))
                }),
                title: item.title,
                hot: false,
                city: item.city,
                direction: direction_of.remove(&item.id).unwrap_or_default(),
                company_id: company_of.get(&item.id).copied(),
                created_at: String::new(),
            })
            .collect();
        let companies = first
            .companies
            .into_iter()
            .map(|(id, title)| ExternalCompany {
                id,
                title,
                logo: logos.remove(&id).unwrap_or_default(),
            })
            .collect();
        Ok((vacancies, companies))
    }

    /// Lists every vacancy under each filter value (`?company=` / `?category=`).
    /// A failed filter only loses that attribution, so errors are logged, not returned.
    async fn collect_filtered(&self, param: &str, ids: &[i64]) -> HashMap<i64, Vec<ListItem>> {
        let mut result: HashMap<i64, Vec<ListItem>> = HashMap::new();
        let mut more_pages = Vec::new();
        let first_pages = ids
            .iter()
            .map(|id| format!("/vacancies?{}={}&page=1", param, id))
            .collect();
        for (path, page) in self.get_many(first_pages).await {
            let Some(id) = filter_value(&path, param) else { continue };
            let Some(parsed) = self.parse_fetched(&path, page) else { continue };
            for p in 2..=parsed.last_page.min(MAX_PAGES) {
                more_pages.push(format!("/vacancies?{}={}&page={}", param, id, p));
            }
            result.entry(id).or_default().extend(parsed.items);
        }
        for (path, page) in self.get_many(more_pages).await {
            let Some(id) = filter_value(&path, param) else { continue };
            if let Some(parsed) = self.parse_fetched(&path, page) {
                result.entry(id).or_default().extend(parsed.items);
            }
        }
        result
    }

    fn parse_fetched(&self, path: &str, page: Result<Option<String>>) -> Option<ListPage> {
        match page {
            Ok(Some(html)) => {
                let parsed = parse_list_page(&html);
                if parsed.is_none() {
                    tracing::warn!("Job board page {} has an unexpected layout", path);
                }
                parsed
            }
            Ok(None) => None,
            Err(e) => {
                tracing::warn!("Job board page {} failed: {}", path, e);
                None
            }
        }
    }

    /// Sanitised description HTML per vacancy, from cache or the detail page.
    async fn vacancy_contents(&self, ids: &[i64]) -> HashMap<i64, String> {
        let mut contents = HashMap::new();
        let mut missing = Vec::new();
        {
            let cache = self.cache.read().await;
            for id in ids {
                match cache.details.get(id) {
                    Some(d) if d.fetched_at.elapsed() < DETAIL_TTL => {
                        contents.insert(*id, d.content.clone());
                    }
                    _ => missing.push(format!("/vacancies/{}", id)),
                }
            }
        }

        let mut fetched = Vec::new();
        for (path, page) in self.get_many(missing).await {
            let Some(id) = vacancy_id_from_href(&path) else { continue };
            match page {
                Ok(Some(html)) => match parse_detail_page(&html) {
                    Some(detail) if !detail.content.trim().is_empty() => {
                        fetched.push((id, ammonia::clean(&detail.content)));
                    }
                    _ => tracing::warn!("Job board vacancy {} page has no description", id),
                },
                Ok(None) => tracing::warn!("Job board vacancy {} is listed but has no page", id),
                Err(e) => tracing::warn!("Job board vacancy {} page failed: {}", id, e),
            }
        }

        let mut cache = self.cache.write().await;
        for (id, content) in fetched {
            cache.details.insert(
                id,
                CachedDetail {
                    content: content.clone(),
                    fetched_at: Instant::now(),
                },
            );
            contents.insert(id, content);
        }
        cache.details.retain(|id, _| ids.contains(id));
        contents
    }

    async fn fetch_detail_page(&self, id: i64) -> Result<Option<ExternalVacancy>> {
        let Some(html) = self.get_page(&format!("/vacancies/{}", id)).await? else {
            return Ok(None);
        };
        let Some(detail) = parse_detail_page(&html) else {
            return Ok(None);
        };
        let created_at = {
            let mut cache = self.cache.write().await;
            cache.first_seen.entry(id).or_insert_with(Utc::now).to_rfc3339()
        };
        Ok(Some(ExternalVacancy {
            id,
            title: detail.title,
            content: ammonia::clean(&detail.content),
            hot: false,
            city: detail.city,
            direction: String::new(),
            company_id: None,
            created_at,
        }))
    }

    async fn get_html(&self, path: &str) -> Result<String> {
        self.get_page(path)
            .await?
            .ok_or_else(|| Error::BadGateway(format!("Job board page {} not found", path)))
    }

    /// GETs a board page; `None` means 404. Transient failures are retried once.
    async fn get_page(&self, path: &str) -> Result<Option<String>> {
        let url = format!("{}{}", self.base_url, path);
        let mut last_error = None;
        for attempt in 0..2 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(700)).await;
            }
            match self
                .client
                .get(&url)
                .header(header::ACCEPT, "text/html")
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => return Ok(Some(resp.text().await?)),
                Ok(resp) if resp.status() == StatusCode::NOT_FOUND => return Ok(None),
                Ok(resp) if resp.status().is_server_error() => {
                    last_error = Some(Error::BadGateway(format!(
                        "Job board returned {} for {}",
                        resp.status(),
                        path
                    )));
                }
                Ok(resp) => {
                    return Err(Error::BadGateway(format!(
                        "Job board returned {} for {}",
                        resp.status(),
                        path
                    )))
                }
                Err(e) => last_error = Some(e.into()),
            }
        }
        Err(last_error.unwrap_or_else(|| Error::BadGateway(format!("Job board request {} failed", path))))
    }

    async fn get_many(&self, paths: Vec<String>) -> Vec<(String, Result<Option<String>>)> {
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
        let mut tasks = tokio::task::JoinSet::new();
        for path in paths {
            let this = self.clone();
            let semaphore = semaphore.clone();
            tasks.spawn(async move {
                let _permit = semaphore.acquire_owned().await;
                let page = this.get_page(&path).await;
                (path, page)
            });
        }
        let mut results = Vec::new();
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok(result) => results.push(result),
                Err(e) => tracing::warn!("Job board fetch task failed: {}", e),
            }
        }
        results
    }
}

#[derive(Debug, Clone)]
struct ListItem {
    id: i64,
    title: String,
    summary: String,
    city: String,
    logo: String,
}

#[derive(Debug)]
struct ListPage {
    items: Vec<ListItem>,
    last_page: u32,
    companies: Vec<(i64, String)>,
    categories: Vec<(i64, String)>,
}

#[derive(Debug)]
struct DetailPage {
    title: String,
    city: String,
    content: String,
}

fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("static CSS selector")
}

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text_in(el: ElementRef, css: &str) -> String {
    el.select(&selector(css))
        .next()
        .map(|e| collapse_whitespace(&e.text().collect::<String>()))
        .unwrap_or_default()
}

/// `None` when the page lacks the vacancy list entirely, i.e. the layout changed.
fn parse_list_page(html: &str) -> Option<ListPage> {
    let doc = Html::parse_document(html);
    doc.select(&selector(".vacancies__list")).next()?;

    let items = doc
        .select(&selector("a.vacancies__link"))
        .filter_map(|a| {
            Some(ListItem {
                id: vacancy_id_from_href(a.value().attr("href")?)?,
                title: text_in(a, ".vacancies__heading"),
                summary: text_in(a, ".vacancies__desc"),
                city: text_in(a, ".vacancies__city"),
                logo: a
                    .select(&selector("img"))
                    .next()
                    .and_then(|img| img.value().attr("src"))
                    .unwrap_or_default()
                    .to_string(),
            })
        })
        .collect();

    let last_page = doc
        .select(&selector("a[href]"))
        .filter_map(|a| page_number(a.value().attr("href")?))
        .max()
        .unwrap_or(1)
        .max(1);

    let labels: HashMap<&str, String> = doc
        .select(&selector("label[for]"))
        .filter_map(|l| Some((l.value().attr("for")?, collapse_whitespace(&l.text().collect::<String>()))))
        .collect();
    let filter_options = |kind: &str| -> Vec<(i64, String)> {
        doc.select(&selector(&format!("input[data-type=\"{}\"]", kind)))
            .filter_map(|input| {
                let id = input.value().attr("value")?.trim().parse().ok()?;
                let key = input.value().attr("id").unwrap_or_default();
                let name = labels
                    .get(key)
                    .filter(|name| !name.is_empty())
                    .cloned()
                    .unwrap_or_else(|| collapse_whitespace(key));
                Some((id, name))
            })
            .collect()
    };

    Some(ListPage {
        items,
        last_page,
        companies: filter_options("company"),
        categories: filter_options("category"),
    })
}

fn parse_detail_page(html: &str) -> Option<DetailPage> {
    let doc = Html::parse_document(html);
    let title = doc
        .select(&selector("h1.vacancy__title"))
        .next()
        .map(|h| collapse_whitespace(&h.text().collect::<String>()))
        .filter(|t| !t.is_empty())?;
    let content = doc
        .select(&selector("main .no-style"))
        .next()
        .map(|d| d.inner_html().trim().to_string())
        .unwrap_or_default();
    let city = doc
        .select(&selector("main.vacancy p.flex"))
        .next()
        .map(|p| collapse_whitespace(&p.text().collect::<String>()))
        .unwrap_or_default();
    Some(DetailPage { title, city, content })
}

fn vacancy_id_from_href(href: &str) -> Option<i64> {
    let path = href.split(['?', '#']).next()?;
    path.rsplit_once("/vacancies/")?.1.trim_end_matches('/').parse().ok()
}

fn query_param<'a>(href: &'a str, name: &str) -> Option<&'a str> {
    let query = href.split_once('?')?.1.split('#').next()?;
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
}

fn page_number(href: &str) -> Option<u32> {
    query_param(href, "page")?.parse().ok()
}

fn filter_value(path: &str, param: &str) -> Option<i64> {
    query_param(path, param)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST_PAGE: &str = r##"<!DOCTYPE html><html><body>
<main class="vacancies container">
  <div class="vacancies__inner"><div>
    <ul class="vacancies__list">
      <li class="vacancies__item">
        <a class="vacancies__link" href="https://job.koinotinav.tj/vacancies/90">
          <img class="vacancies__logo" src="https://job.koinotinav.tj/storage/images/companies/a.png" alt="">
          <div class="vacancies__heading"><h1>OTC Продакт-менеджер</h1></div>
          <div class="vacancies__desc">Хотите развивать бренды</div>
          <p class="vacancies__city"><svg width="16" height="20"><use xlink:href="#location" /></svg>
              Душанбе
          </p>
        </a>
      </li>
      <li class="vacancies__item">
        <a class="vacancies__link" href="https://job.koinotinav.tj/vacancies/89">
          <img class="vacancies__logo" src="" alt="">
          <div class="vacancies__heading"><h1><strong>RX Продакт-менеджер</strong></h1></div>
          <div class="vacancies__desc">Описание &amp; детали</div>
          <p class="vacancies__city">Dushanbe</p>
        </a>
      </li>
    </ul>
    <ul class="pagination">
      <li><a href="https://job.koinotinav.tj/vacancies?page=2">2</a></li>
      <li><a href="https://job.koinotinav.tj/vacancies?page=8#vacancies">8</a></li>
    </ul>
  </div>
  <aside class="vacancies__filter"><form>
    <fieldset><legend>Сфера</legend><ul>
      <li><input id="IT и разработка" type="checkbox" value="3" data-type="category">
          <label for="IT и разработка">
              IT и разработка
          </label></li>
    </ul></fieldset>
    <fieldset><legend>Компании</legend><ul>
      <li><input id="КОИНОТИ НАВ" type="checkbox" value="1" data-type="company">
          <label for="КОИНОТИ НАВ">КОИНОТИ НАВ</label></li>
      <li><input id="AtS Тренинг Центр" type="checkbox" value="2" data-type="company"></li>
    </ul></fieldset>
  </form></aside></div>
</main></body></html>"##;

    const DETAIL_PAGE: &str = r##"<!DOCTYPE html><html><body>
<main class="vacancy container">
  <h1 class="vacancy__title title">RX Продакт-менеджер</h1>
  <div class="flex flex-col gap-10">
    <div>
      <p class="flex mt-0 mb-5 text-[#73787D] text-[14px] gap-2"><svg></svg>
          Душанбе
      </p>
      <div class="no-style">
        <p>Позиция <strong>RX Продакт-менеджера</strong></p><script>alert(1)</script>
        <ol start="1"><li><p>Разрабатывать стратегию</p></li></ol>
        <p><a href="mailto:hr@koinotinav.tj" onclick="x()">hr@koinotinav.tj</a></p>
      </div>
    </div>
    <form class="resume-form"><h1 class="resume-form__title">Отклик на вакансию</h1></form>
  </div>
</main></body></html>"##;

    #[test]
    fn list_page_yields_items_pagination_and_filters() {
        let page = parse_list_page(LIST_PAGE).expect("list layout recognised");

        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].id, 90);
        assert_eq!(page.items[0].title, "OTC Продакт-менеджер");
        assert_eq!(page.items[0].city, "Душанбе");
        assert!(page.items[0].logo.ends_with("/a.png"));
        assert_eq!(page.items[1].title, "RX Продакт-менеджер");
        assert_eq!(page.items[1].summary, "Описание & детали");
        assert_eq!(page.last_page, 8);
        assert_eq!(page.categories, vec![(3, "IT и разработка".to_string())]);
        assert_eq!(
            page.companies,
            vec![(1, "КОИНОТИ НАВ".to_string()), (2, "AtS Тренинг Центр".to_string())]
        );
    }

    #[test]
    fn a_page_without_the_vacancy_list_is_rejected() {
        assert!(parse_list_page("<html><body><h1>Not Found</h1></body></html>").is_none());
    }

    #[test]
    fn an_empty_filter_result_is_still_a_valid_page() {
        let page = parse_list_page(r#"<ul class="vacancies__list"></ul>"#).unwrap();
        assert!(page.items.is_empty());
        assert_eq!(page.last_page, 1);
    }

    #[test]
    fn detail_page_yields_title_city_and_description() {
        let page = parse_detail_page(DETAIL_PAGE).expect("detail layout recognised");
        assert_eq!(page.title, "RX Продакт-менеджер");
        assert_eq!(page.city, "Душанбе");
        assert!(page.content.contains("Разрабатывать стратегию"));
        assert!(!page.content.contains("Отклик на вакансию"));
    }

    #[test]
    fn description_html_is_sanitised() {
        let page = parse_detail_page(DETAIL_PAGE).unwrap();
        let clean = ammonia::clean(&page.content);
        assert!(!clean.contains("<script"));
        assert!(!clean.contains("onclick"));
        assert!(clean.contains("<strong>RX Продакт-менеджера</strong>"));
        assert!(clean.contains("<li>"));
    }

    #[test]
    fn a_not_found_page_is_not_a_vacancy() {
        assert!(parse_detail_page("<html><title>Not Found</title></html>").is_none());
    }

    #[tokio::test]
    async fn live_board_snapshot_is_complete() {
        if std::env::var("RUN_LIVE_JOB_BOARD").is_err() {
            eprintln!("skipping: set RUN_LIVE_JOB_BOARD=1 to run");
            return;
        }
        let service = KoinotinavService::new();
        let vacancies = service.fetch_vacancies().await.expect("board reachable");
        let companies = service.fetch_companies().await.unwrap();
        assert!(!vacancies.is_empty());
        assert!(!companies.is_empty());
        for v in &vacancies {
            assert!(!v.title.is_empty(), "vacancy {} has no title", v.id);
            assert!(!v.content.trim().is_empty(), "vacancy {} has no content", v.id);
            assert!(DateTime::parse_from_rfc3339(&v.created_at).is_ok());
        }
        let with_company = vacancies.iter().filter(|v| v.company_id.is_some()).count();
        let with_direction = vacancies.iter().filter(|v| !v.direction.is_empty()).count();
        eprintln!(
            "{} vacancies ({} with company, {} with direction), {} companies",
            vacancies.len(),
            with_company,
            with_direction,
            companies.len()
        );

        let first = &vacancies[0];
        let found = service.fetch_vacancy(first.id).await.unwrap().unwrap();
        assert_eq!(found.title, first.title);
        assert!(service.fetch_vacancy(99_999_999).await.unwrap().is_none());
    }

    #[test]
    fn hrefs_and_query_strings_are_parsed() {
        assert_eq!(vacancy_id_from_href("https://job.koinotinav.tj/vacancies/89"), Some(89));
        assert_eq!(vacancy_id_from_href("/vacancies/89/?ref=x"), Some(89));
        assert_eq!(vacancy_id_from_href("https://job.koinotinav.tj/vacancies"), None);
        assert_eq!(page_number("https://job.koinotinav.tj/vacancies?page=8#vacancies"), Some(8));
        assert_eq!(page_number("?city=Dushanbe&page=3"), Some(3));
        assert_eq!(page_number("/vacancies?pages=3"), None);
        assert_eq!(filter_value("/vacancies?company=12&page=1", "company"), Some(12));
        assert_eq!(filter_value("/vacancies?category=4&page=2", "company"), None);
    }
}
