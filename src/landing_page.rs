use crate::awards::award_catalog;
use crate::config::creator_link_for_base;
use crate::error::AppError;
use crate::ports::AwardRepository;

const HISTORY_LIMIT: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicRoute {
    LandingPage,
    Health,
    NotFound,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AwardHistoryEntry {
    pub period_key: String,
    pub winner_name: String,
    pub winner_picture: Option<String>,
    pub loops: Option<f64>,
    pub profile_url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AwardHistorySection {
    pub title: &'static str,
    pub description: &'static str,
    pub entries: Vec<AwardHistoryEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LandingPageView {
    pub sections: Vec<AwardHistorySection>,
}

pub fn route_path(path: &str) -> PublicRoute {
    match path {
        "/" => PublicRoute::LandingPage,
        "/healthz" => PublicRoute::Health,
        _ => PublicRoute::NotFound,
    }
}

pub async fn build_view<R: AwardRepository>(
    repository: &R,
    creator_base_url: &str,
) -> Result<LandingPageView, AppError> {
    let mut sections = Vec::new();

    for award in award_catalog() {
        let runs = repository
            .load_recent_completed_runs(award.slug, HISTORY_LIMIT)
            .await?;

        let entries = runs
            .into_iter()
            .filter_map(|run| {
                let pubkey = run.winner_pubkey?;
                Some(AwardHistoryEntry {
                    period_key: run.period_key,
                    winner_name: run
                        .winner_display_name
                        .or(run.winner_name)
                        .unwrap_or_else(|| pubkey.chars().take(8).collect()),
                    winner_picture: run
                        .winner_picture
                        .and_then(|picture| (!picture.trim().is_empty()).then_some(picture)),
                    loops: run.loops,
                    profile_url: creator_link_for_base(
                        creator_base_url,
                        run.winner_nip05.as_deref(),
                        &pubkey,
                    ),
                })
            })
            .collect();

        sections.push(AwardHistorySection {
            title: award.badge_name,
            description: award.description,
            entries,
        });
    }

    Ok(LandingPageView { sections })
}

pub fn render_page(view: &LandingPageView) -> String {
    let sections = view
        .sections
        .iter()
        .map(render_section)
        .collect::<Vec<_>>()
        .join("");

    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Divine Badges</title><style>{}</style></head><body><main class=\"page\"><section class=\"hero\"><p class=\"eyebrow\">Divine Badges</p><h1>Automated creator awards for the Divine leaderboard.</h1><p class=\"lede\">Diviner awards recognize the top active creators from the most recently closed UTC day, week, and month.</p></section><section class=\"how\"><div><h2>How it works</h2><p>The worker closes periods on UTC boundaries, reads the Divine creator leaderboard, filters for active creators, and publishes the result as Nostr badge awards.</p></div><div><p>Each section below shows the latest completed winners pulled from D1, newest first.</p></div></section><section class=\"awards\">{sections}</section><footer class=\"footer\"><a href=\"https://divine.video\">Divine</a><a href=\"/healthz\">Health</a><span>Cloudflare Worker + D1 + Nostr</span></footer></main></body></html>",
        page_css()
    )
}

fn render_section(section: &AwardHistorySection) -> String {
    let entries = if section.entries.is_empty() {
        "<li class=\"empty\">No awards issued yet.</li>".to_string()
    } else {
        section
            .entries
            .iter()
            .map(render_entry)
            .collect::<Vec<_>>()
            .join("")
    };

    format!(
        "<article class=\"award\"><h2>{}</h2><p>{}</p><ol>{}</ol></article>",
        escape_html(section.title),
        escape_html(section.description),
        entries
    )
}

fn render_entry(entry: &AwardHistoryEntry) -> String {
    let media = if let Some(picture) = &entry.winner_picture {
        format!(
            "<img class=\"avatar\" src=\"{}\" alt=\"{} avatar\">",
            escape_html(picture),
            escape_html(&entry.winner_name)
        )
    } else {
        format!(
            "<div class=\"avatar placeholder\">{}</div>",
            escape_html(&initials(&entry.winner_name))
        )
    };
    let loops = entry
        .loops
        .map(|value| {
            format!(
                "<span class=\"metric\">{} loops</span>",
                format_loops(value)
            )
        })
        .unwrap_or_default();

    format!(
        "<li class=\"winner\">{media}<div class=\"copy\"><span class=\"period\">{}</span><a href=\"{}\">{}</a>{loops}</div></li>",
        escape_html(&entry.period_key),
        escape_html(&entry.profile_url),
        escape_html(&entry.winner_name),
    )
}

fn initials(name: &str) -> String {
    name.chars()
        .find(|character| !character.is_whitespace())
        .map(|character| character.to_uppercase().collect())
        .unwrap_or_else(|| "?".into())
}

fn format_loops(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value:.1}")
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn page_css() -> &'static str {
    ":root{color-scheme:light;font-family:Georgia,'Iowan Old Style',serif;background:#f4efe6;color:#1f241f}*{box-sizing:border-box}body{margin:0;background:radial-gradient(circle at top,#f9f3e8 0,#f4efe6 45%,#e7e4d8 100%)}a{color:#173f35}main.page{max-width:1120px;margin:0 auto;padding:48px 20px 64px}.hero,.how,.award,.footer{backdrop-filter:blur(8px)}.hero{padding:40px;border-radius:28px;background:rgba(255,252,245,.86);box-shadow:0 18px 40px rgba(36,45,36,.08)}.eyebrow{text-transform:uppercase;letter-spacing:.12em;font-size:.75rem;color:#6e5b3f}.hero h1{font-size:clamp(2.5rem,5vw,4.75rem);line-height:.96;margin:12px 0 16px}.lede{max-width:42rem;font-size:1.1rem;line-height:1.6}.how{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:16px;margin-top:20px;padding:24px;border-radius:24px;background:rgba(255,252,245,.72)}.awards{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:18px;margin-top:22px}.award{padding:22px;border-radius:24px;background:rgba(255,252,245,.88);box-shadow:0 16px 30px rgba(36,45,36,.08)}.award h2{margin:0 0 10px;font-size:1.55rem}.award p{margin:0 0 18px;line-height:1.5;color:#4e544b}.award ol{list-style:none;padding:0;margin:0;display:grid;gap:12px}.winner{display:flex;gap:12px;align-items:center;padding:12px;border-radius:18px;background:#f1eadf}.avatar{width:52px;height:52px;border-radius:16px;object-fit:cover;background:#d8d2c3}.avatar.placeholder{display:grid;place-items:center;font-weight:700;color:#173f35}.copy{display:grid;gap:2px}.period{font-size:.78rem;letter-spacing:.08em;text-transform:uppercase;color:#6b665a}.metric{font-size:.92rem;color:#4e544b}.empty{padding:18px;border-radius:18px;background:#f1eadf;color:#4e544b}.footer{display:flex;gap:16px;flex-wrap:wrap;align-items:center;margin-top:24px;padding:18px 4px;color:#4e544b}"
}
