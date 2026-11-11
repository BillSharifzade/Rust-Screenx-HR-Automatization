//! Client for the Koinotinav job board API (`job.koinotinav.tj/api/v1`).
//!
//! Vacancies and companies are public. Candidate applications and their resume
//! files need the `X-Api-Key` issued by the board (`KOINOTINAV_API_KEY`).
//!
//! Callers (dashboard, candidate registration, OneF endpoints, AI scoring) all
//! read one in-memory snapshot that a background task keeps warm. When the board
//! is unreachable the last good snapshot keeps being served instead of failing
//! every page that lists vacancies.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use chrono::{DateTime, SecondsFormat, Utc};
use reqwest::{header, Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

use crate::error::{Error, Result};

const DEFAULT_BASE_URL: &str = "https://job.koinotinav.tj";
const API_PREFIX: &str = "/api/v1";
/// How long a snapshot is served before it is rebuilt.
const SNAPSHOT_TTL: Duration = Duration::from_secs(10 * 60);
/// Vacancies looked up by id because they are not on the public list (closed ones).
const LOOKUP_TTL: Duration = Duration::from_secs(30 * 60);
/// The API caps `per_page` at 100.
const PER_PAGE: u32 = 100;
/// Guard against following a broken pagination forever.
const MAX_PAGES: u32 = 50;
const MAX_RESUME_BYTES: usize = 20 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalVacancy {
    pub id: i64,
    pub title: String,
    /// Sanitised description HTML.
    pub content: String,
    pub hot: bool,
    pub city: String,
    /// Category name ("Сфера" on the site).
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

/// A candidate's application left on the board (`GET /applications`).
#[derive(Debug, Clone, Deserialize)]
pub struct PortalApplication {
    pub id: i64,
    pub vacancy: Option<PortalApplicationVacancy>,
    /// `resume_form` or `telegram_form`.
    pub source: Option<String>,
    pub status: Option<String>,
    pub name: Option<String>,
    /// `YYYY-MM-DD`.
    pub birthdate: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub city: Option<String>,
    pub about: Option<String>,
    pub locale: Option<String>,
    pub consent: Option<bool>,
    pub resume: Option<PortalResume>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PortalApplicationVacancy {
    pub id: i64,
    pub title: Option<String>,
    pub external_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PortalResume {
    /// Original file name as uploaded by the candidate.
    pub name: Option<String>,
}

#[derive(Deserialize)]
struct Page<T> {
    data: Vec<T>,
    meta: Option<PageMeta>,
}

#[derive(Deserialize)]
struct PageMeta {
    last_page: u32,
}

#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

#[derive(Deserialize)]
struct ApiVacancy {
    id: i64,
    title: Option<String>,
    summary: Option<String>,
    description_html: Option<String>,
    #[serde(default)]
    is_hot: bool,
    city: Option<String>,
    company: Option<ApiRef>,
    category: Option<ApiRef>,
    published_at: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Deserialize)]
struct ApiRef {
    id: i64,
    name: Option<String>,
}

#[derive(Deserialize)]
struct ApiCompany {
    id: i64,
    name: Option<String>,
    logo_url: Option<String>,
}

impl From<ApiVacancy> for ExternalVacancy {
    fn from(v: ApiVacancy) -> Self {
        let content = match v.description_html.filter(|h| !h.trim().is_empty()) {
            Some(html) => ammonia::clean(&html),
            None => format!("<p>{}</p>", escape_text(v.summary.as_deref().unwrap_or("").trim())),
        };
        ExternalVacancy {
            id: v.id,
            title: v.title.unwrap_or_default().trim().to_string(),
            content,
            hot: v.is_hot,
            city: v.city.unwrap_or_default().trim().to_string(),
            direction: v.category.and_then(|c| c.name).unwrap_or_default().trim().to_string(),
            company_id: v.company.map(|c| c.id),
            // The frontend formats this date and cannot take "".
            created_at: [v.published_at, v.created_at, v.updated_at]
                .into_iter()
                .flatten()
                .find(|d| DateTime::parse_from_rfc3339(d).is_ok())
                .unwrap_or_else(|| Utc::now().to_rfc3339()),
        }
    }
}

fn escape_text(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[derive(Clone)]
pub struct KoinotinavService {
    client: Client,
    base_url: Arc<str>,
    api_key: Option<Arc<str>>,
    cache: Arc<RwLock<Cache>>,
    refresh_lock: Arc<Mutex<()>>,
}

#[derive(Default)]
struct Cache {
    snapshot: Option<Arc<Snapshot>>,
    lookups: HashMap<i64, CachedLookup>,
}

struct Snapshot {
    vacancies: Vec<ExternalVacancy>,
    companies: Vec<ExternalCompany>,
    fetched_at: Instant,
}

struct CachedLookup {
    vacancy: Option<ExternalVacancy>,
    fetched_at: Instant,
}

impl Default for KoinotinavService {
    fn default() -> Self {
        Self::new()
    }
}

impl KoinotinavService {
    pub fn new() -> Self {
        let env = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };
        let base_url = env("KOINOTINAV_BASE_URL")
            .map(|s| s.trim_end_matches('/').to_string())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("ScreenX-HR/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("failed to build job board HTTP client");

        Self {
            client,
            base_url: base_url.into(),
            api_key: env("KOINOTINAV_API_KEY").map(Into::into),
            cache: Arc::new(RwLock::new(Cache::default())),
            refresh_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Applications and resumes can only be read with an API key.
    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
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
        if id <= 0 {
            return Ok(None);
        }
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

        // Closed vacancies leave the public list but are still served by id.
        if let Some(hit) = self.cache.read().await.lookups.get(&id) {
            if hit.fetched_at.elapsed() < LOOKUP_TTL {
                return Ok(hit.vacancy.clone());
            }
        }
        let vacancy = self
            .get_json::<Envelope<ApiVacancy>>(&format!("/vacancies/{}", id), &[], false)
            .await?
            .map(|e| ExternalVacancy::from(e.data));
        let mut cache = self.cache.write().await;
        cache.lookups.retain(|_, hit| hit.fetched_at.elapsed() < LOOKUP_TTL);
        cache.lookups.insert(
            id,
            CachedLookup {
                vacancy: vacancy.clone(),
                fetched_at: Instant::now(),
            },
        );
        Ok(vacancy)
    }

    /// Every application received since `since` (all of them when `None`), oldest page first.
    pub async fn fetch_applications(
        &self,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<PortalApplication>> {
        let mut query = vec![("per_page", PER_PAGE.to_string())];
        if let Some(since) = since {
            query.push(("since", since.to_rfc3339_opts(SecondsFormat::Secs, true)));
        }
        self.get_all_pages("/applications", query, true).await
    }

    /// The resume file attached to an application; `None` when there is none.
    pub async fn fetch_resume(&self, application_id: i64) -> Result<Option<Bytes>> {
        let path = format!("/applications/{}/resume", application_id);
        let Some(resp) = self.get(&path, &[], true, "*/*").await? else {
            return Ok(None);
        };
        if resp
            .content_length()
            .is_some_and(|len| len as usize > MAX_RESUME_BYTES)
        {
            return Err(Error::BadGateway(format!("{} is larger than 20 MB", path)));
        }
        let bytes = resp.bytes().await?;
        if bytes.len() > MAX_RESUME_BYTES {
            return Err(Error::BadGateway(format!("{} is larger than 20 MB", path)));
        }
        Ok(Some(bytes))
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
        let (vacancies, companies) = tokio::try_join!(
            self.get_all_pages::<ApiVacancy>("/vacancies", vec![("per_page", PER_PAGE.to_string())], false),
            self.get_json::<Envelope<Vec<ApiCompany>>>("/companies", &[], false),
        )?;
        let mut vacancies: Vec<ExternalVacancy> =
            vacancies.into_iter().map(ExternalVacancy::from).collect();
        let mut seen = std::collections::HashSet::new();
        vacancies.retain(|v| seen.insert(v.id));
        let companies: Vec<ExternalCompany> = companies
            .ok_or_else(|| Error::BadGateway("Job board /companies not found".into()))?
            .data
            .into_iter()
            .map(|c| ExternalCompany {
                id: c.id,
                title: c.name.unwrap_or_default().trim().to_string(),
                logo: c.logo_url.unwrap_or_default(),
            })
            .collect();

        tracing::info!(
            "Job board snapshot: {} vacancies, {} companies ({} ms)",
            vacancies.len(),
            companies.len(),
            started.elapsed().as_millis()
        );
        let snapshot = Arc::new(Snapshot {
            vacancies,
            companies,
            fetched_at: Instant::now(),
        });
        self.cache.write().await.snapshot = Some(snapshot.clone());
        Ok(snapshot)
    }

    /// Follows `meta.last_page`. A missing page fails the whole call rather than
    /// silently dropping records.
    async fn get_all_pages<T: DeserializeOwned>(
        &self,
        path: &str,
        mut query: Vec<(&'static str, String)>,
        keyed: bool,
    ) -> Result<Vec<T>> {
        let mut items = Vec::new();
        let mut page = 1;
        loop {
            query.retain(|(k, _)| *k != "page");
            query.push(("page", page.to_string()));
            let resp: Page<T> = self
                .get_json(path, &query, keyed)
                .await?
                .ok_or_else(|| Error::BadGateway(format!("Job board {} not found", path)))?;
            items.extend(resp.data);
            let last_page = resp.meta.map(|m| m.last_page).unwrap_or(1);
            if page >= last_page || page >= MAX_PAGES {
                return Ok(items);
            }
            page += 1;
        }
    }

    /// GETs and decodes an API path; `None` means 404.
    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        keyed: bool,
    ) -> Result<Option<T>> {
        let Some(resp) = self.get(path, query, keyed, "application/json").await? else {
            return Ok(None);
        };
        let body = resp.bytes().await?;
        serde_json::from_slice(&body).map(Some).map_err(|e| {
            Error::BadGateway(format!("Job board sent an unexpected {} response: {}", path, e))
        })
    }

    /// GETs an API path; `None` means 404. Server errors and network failures
    /// are retried once.
    async fn get(
        &self,
        path: &str,
        query: &[(&str, String)],
        keyed: bool,
        accept: &str,
    ) -> Result<Option<Response>> {
        let url = format!("{}{}{}", self.base_url, API_PREFIX, path);
        let key = match (keyed, &self.api_key) {
            (false, _) => None,
            (true, Some(key)) => Some(key.clone()),
            (true, None) => return Err(Error::Config("KOINOTINAV_API_KEY is not set".into())),
        };
        let mut last_error = None;
        for attempt in 0..2 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(700)).await;
            }
            let mut request = self
                .client
                .get(&url)
                .query(query)
                .header(header::ACCEPT, accept);
            if let Some(key) = &key {
                request = request.header("X-Api-Key", key.as_ref());
            }
            match request.send().await {
                Ok(resp) if resp.status().is_success() => return Ok(Some(resp)),
                Ok(resp) if resp.status() == StatusCode::NOT_FOUND => return Ok(None),
                Ok(resp) if matches!(resp.status(), StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) => {
                    return Err(Error::BadGateway(format!(
                        "Job board rejected the API key ({}) for {}",
                        resp.status(),
                        path
                    )))
                }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    const VACANCY: &str = r##"{
        "id": 99,
        "title": " Главный бухгалтер ",
        "summary": "Коротко",
        "description_html": "<p>Ищем <strong>бухгалтера</strong></p><script>alert(1)</script><a href=\"/x\" onclick=\"x()\">a</a>",
        "description_text": "Ищем бухгалтера",
        "status": "published",
        "is_hot": true,
        "city": "Душанбе",
        "company": {"id": 1, "name": "КОИНОТИ НАВ", "logo_url": null},
        "category": {"id": 3, "name": "IT и разработка"},
        "url": "https://job.koinotinav.tj/vacancies/glavnyy-bukhgalter",
        "external_id": null,
        "published_at": "2026-10-31T10:25:10+05:00",
        "created_at": "2026-11-02T17:11:25+05:00",
        "updated_at": "2026-11-02T17:11:25+05:00",
        "closed_at": null
    }"##;

    #[test]
    fn api_vacancy_maps_onto_the_shape_the_frontend_reads() {
        let v: ApiVacancy = serde_json::from_str(VACANCY).unwrap();
        let v = ExternalVacancy::from(v);
        assert_eq!(v.id, 99);
        assert_eq!(v.title, "Главный бухгалтер");
        assert!(v.hot);
        assert_eq!(v.city, "Душанбе");
        assert_eq!(v.direction, "IT и разработка");
        assert_eq!(v.company_id, Some(1));
        assert_eq!(v.created_at, "2026-10-31T10:25:10+05:00");
        assert!(v.content.contains("<strong>бухгалтера</strong>"));
        assert!(!v.content.contains("<script"));
        assert!(!v.content.contains("onclick"));
    }

    #[test]
    fn sparse_vacancy_still_gets_a_description_and_a_date() {
        let v: ApiVacancy = serde_json::from_str(
            r#"{"id": 5, "title": null, "summary": "Только <кратко>", "description_html": "",
                "city": null, "company": null, "category": null,
                "published_at": null, "created_at": "not a date", "updated_at": null}"#,
        )
        .unwrap();
        let v = ExternalVacancy::from(v);
        assert_eq!(v.content, "<p>Только &lt;кратко&gt;</p>");
        assert_eq!(v.title, "");
        assert_eq!(v.company_id, None);
        assert!(DateTime::parse_from_rfc3339(&v.created_at).is_ok());
    }

    #[test]
    fn list_page_and_application_decode() {
        let page: Page<ApiVacancy> = serde_json::from_str(&format!(
            r#"{{"data": [{}], "links": {{}}, "meta": {{"current_page": 1, "last_page": 3, "per_page": 100, "total": 201}}}}"#,
            VACANCY
        ))
        .unwrap();
        assert_eq!(page.data.len(), 1);
        assert_eq!(page.meta.unwrap().last_page, 3);

        let app: PortalApplication = serde_json::from_str(
            r#"{"id": 14, "vacancy": {"id": 78, "title": "Юрист", "external_id": null, "url": "u"},
                "source": "resume_form", "status": "new", "name": "Иван", "birthdate": "1990-05-01",
                "phone": "992900000000", "email": "a@b.tj", "city": "Душанбе", "about": null,
                "locale": "ru", "consent": true, "resume": {"name": "cv.pdf", "url": "u"},
                "created_at": "2026-11-04T12:08:10+05:00", "updated_at": "2026-11-04T12:08:10+05:00"}"#,
        )
        .unwrap();
        assert_eq!(app.vacancy.unwrap().id, 78);
        assert_eq!(app.resume.unwrap().name.as_deref(), Some("cv.pdf"));
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

        if service.has_api_key() {
            let apps = service.fetch_applications(None).await.expect("key accepted");
            eprintln!("{} applications", apps.len());
            if let Some(app) = apps.iter().find(|a| a.resume.is_some()) {
                let file = service.fetch_resume(app.id).await.unwrap();
                assert!(file.is_some_and(|f| !f.is_empty()));
            }
        }
    }
}
