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
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Diviner Awards · Divine</title><meta name="description" content="Badges for the loudest humans on Divine. No algorithm picks. No vibes check. Just loops."><link rel="preconnect" href="https://fonts.googleapis.com"><link rel="preconnect" href="https://fonts.gstatic.com" crossorigin><link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Bricolage+Grotesque:opsz,wght@12..96,500;12..96,700;12..96,800&family=Inter:wght@400;500;600;700&display=swap"><style>{css}</style></head><body><div class="splat splat--green"></div><div class="splat splat--pink"></div><div class="splat splat--yellow"></div><main class="shell"><nav class="topbar"><a class="brand" href="https://divine.video"><span class="logomark" aria-hidden="true"></span><span>Divine</span></a><span class="nav-right"><a class="me-link" href="/me">My badges →</a><span class="status"><span class="dot" aria-hidden="true"></span>Live &middot; updated daily</span></span></nav><header class="hero"><p class="eyebrow"><span class="sticker sticker--yellow">No slop. All human.</span></p><h1>Trophies for the loud<span class="punct">.</span></h1><p class="lede">Every day, every week, every month, we hand a badge to the loudest creator on Divine. No algorithm picks. No vibes check. Just loops. This page is the receipt.</p></header>{sections}<section class="howto"><p class="eyebrow eyebrow--mint">How we pick the loud</p><h2>Loops talk. We listen.</h2><ol><li><span class="n">1</span><p><b>We watch the loops.</b> Every morning we look at who&rsquo;s been racking up loops on Divine for the day, the week, and the month.</p></li><li><span class="n">2</span><p><b>Active humans only.</b> We skip anyone who hasn&rsquo;t posted a video in the last 30 days. Trophies are for the creators showing up.</p></li><li><span class="n">3</span><p><b>Badge hits the wallet.</b> The loudest creator in each window gets a signed Diviner badge on their Nostr profile, and a shout-out in our Discord.</p></li></ol></section><footer class="foot"><p>Built at <a href="https://divine.video">divine.video</a>. Open source. Own what you make. Life in loops.</p><nav><a href="/healthz">health</a><a href="https://github.com/divinevideo/divine-badges">source</a><a href="https://divine.video">divine.video</a></nav></footer></main></body></html>"#,
        css = page_css()
    )
}

fn render_section(section: &AwardHistorySection) -> String {
    let (slug_class, eyebrow, deck) = match section.title {
        "Diviner of the Day" => (
            "daily",
            "Daily drop",
            "Whoever yesterday&rsquo;s loops loved most.",
        ),
        "Diviner of the Week" => ("weekly", "Weekly drop", "Seven days of loops. One badge."),
        "Diviner of the Month" => (
            "monthly",
            "Monthly drop",
            "A month of loud. One human at the top.",
        ),
        _ => ("custom", "Drop", section.description),
    };

    let entries = if section.entries.is_empty() {
        "<li class=\"winner winner--empty\">Nothing here yet. Go make some noise.</li>".to_string()
    } else {
        section
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| render_entry(entry, index))
            .collect::<Vec<_>>()
            .join("")
    };

    format!(
        r#"<section class="award award--{slug}"><header class="award-head"><p class="eyebrow">{eyebrow}</p><h2>{title}</h2><p class="deck">{deck}</p></header><ol class="winners">{entries}</ol></section>"#,
        slug = slug_class,
        eyebrow = eyebrow,
        title = escape_html(section.title),
        deck = deck,
        entries = entries
    )
}

fn render_entry(entry: &AwardHistoryEntry, rank: usize) -> String {
    let media = if let Some(picture) = &entry.winner_picture {
        format!(
            r#"<div class="badge"><img src="{src}" alt="" loading="lazy"></div>"#,
            src = escape_html(picture),
        )
    } else {
        format!(
            r#"<div class="badge"><span class="placeholder">{initial}</span></div>"#,
            initial = escape_html(&initials(&entry.winner_name)),
        )
    };

    let stamp = if rank == 0 {
        r#"<span class="stamp">Current</span>"#
    } else {
        ""
    };

    let loops = match entry.loops {
        Some(value) => format!(
            r#"<div class="score"><span class="num">{}</span><span class="unit">loops</span></div>"#,
            format_loops(value)
        ),
        None => String::new(),
    };

    let rank_class = if rank == 0 { " winner--first" } else { "" };

    format!(
        r#"<li class="winner{rank_class}">{media}<div class="body">{stamp}<span class="period">{period}</span><a class="name" href="{url}">{name}</a></div>{loops}</li>"#,
        rank_class = rank_class,
        media = media,
        stamp = stamp,
        period = escape_html(&entry.period_key),
        url = escape_html(&entry.profile_url),
        name = escape_html(&entry.winner_name),
        loops = loops,
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
    r#":root{--green:#27C58B;--dark:#07241B;--mint:#D0FBCB;--off:#F9F7F6;--yellow:#FFF140;--pink:#FF7FAF;--orange:#FF7640;--violet:#A3A9FF;--purple:#8568FF;color-scheme:dark}
*{box-sizing:border-box;margin:0;padding:0}
html,body{background:var(--dark);color:var(--off);font-family:'Inter',system-ui,-apple-system,sans-serif;font-size:17px;line-height:1.55;-webkit-font-smoothing:antialiased;text-rendering:geometricPrecision}
body{position:relative;overflow-x:hidden;min-height:100vh}
a{color:inherit;text-decoration:none}
img{display:block;max-width:100%}
h1,h2,h3,.display{font-family:'Bricolage Grotesque','Inter',sans-serif;font-weight:800;letter-spacing:-.02em;line-height:.95}
.shell{position:relative;z-index:2;max-width:1240px;margin:0 auto;padding:0 clamp(20px,4vw,48px)}
.splat{position:absolute;z-index:0;pointer-events:none;border-radius:50%;filter:blur(.5px)}
.splat--green{top:120px;right:-140px;width:340px;height:340px;background:var(--green);opacity:.9}
.splat--pink{top:calc(100vh + 120px);left:-110px;width:260px;height:260px;background:var(--pink);opacity:.85}
.splat--yellow{top:calc(180vh + 40px);right:-60px;width:200px;height:200px;background:var(--yellow);opacity:.8}
.topbar{display:flex;justify-content:space-between;align-items:center;padding:28px 0;gap:16px;flex-wrap:wrap}
.nav-right{display:inline-flex;align-items:center;gap:14px;flex-wrap:wrap}
.me-link{font-family:'Bricolage Grotesque',sans-serif;font-weight:700;font-size:.88rem;color:var(--off);padding:8px 14px;border:2px solid var(--green);border-radius:999px;transition:background .15s,color .15s}
.me-link:hover{background:var(--green);color:var(--dark)}
.brand{display:inline-flex;align-items:center;gap:12px;color:var(--off);font-family:'Bricolage Grotesque',sans-serif;font-weight:800;font-size:1.35rem;letter-spacing:-.01em}
.logomark{width:26px;height:26px;border-radius:50%;background:var(--green);border:2px solid var(--off);position:relative;display:inline-block}
.logomark::after{content:"";position:absolute;inset:5px;background:var(--dark);border-radius:50%}
.status{display:inline-flex;align-items:center;gap:10px;padding:8px 14px;border:1.5px solid var(--mint);border-radius:999px;font-weight:500;font-size:.85rem;color:var(--mint)}
.status .dot{width:8px;height:8px;border-radius:50%;background:var(--green);animation:pulse 1.8s ease-in-out infinite}
@keyframes pulse{0%,100%{opacity:1;transform:scale(1)}50%{opacity:.45;transform:scale(1.35)}}
.eyebrow{font-family:'Inter',sans-serif;font-weight:600;font-size:.92rem;color:var(--mint);margin:0}
.eyebrow--mint{color:var(--mint)}
.sticker{display:inline-block;padding:10px 18px;border-radius:999px;font-family:'Bricolage Grotesque',sans-serif;font-weight:700;font-size:1rem;color:var(--dark);border:2px solid var(--dark);transform:rotate(-3deg);box-shadow:4px 4px 0 var(--dark)}
.sticker--yellow{background:var(--yellow)}
.hero{padding:clamp(36px,6vw,88px) 0 clamp(48px,8vw,120px);position:relative}
.hero .eyebrow{margin-bottom:28px}
.hero h1{color:var(--off);font-size:clamp(3.2rem,11vw,10rem);max-width:16ch}
.hero h1 .punct{color:var(--green)}
.hero .lede{max-width:58ch;margin-top:32px;font-size:clamp(1.1rem,1.3vw,1.4rem);line-height:1.5;color:var(--mint);font-weight:400}
.hero-meta{margin-top:36px;display:flex;gap:14px 32px;flex-wrap:wrap;color:var(--mint);font-size:.92rem;font-weight:500}
.hero-meta b{color:var(--off);font-weight:700}
.award{padding:clamp(40px,7vw,88px) 0;border-top:2px solid rgba(208,251,203,.14);position:relative}
.award-head{display:grid;gap:14px;max-width:62ch;margin-bottom:36px}
.award .eyebrow{color:var(--green);font-weight:700}
.award--weekly .eyebrow{color:var(--yellow)}
.award--monthly .eyebrow{color:var(--pink)}
.award h2{color:var(--off);font-size:clamp(2.4rem,5vw,4.4rem)}
.award .deck{color:var(--mint);font-family:'Bricolage Grotesque',sans-serif;font-weight:500;font-size:clamp(1.25rem,1.7vw,1.7rem);line-height:1.25}
.winners{list-style:none;display:grid;gap:16px}
.winner{display:grid;grid-template-columns:auto 1fr auto;gap:24px;align-items:center;padding:22px 26px;background:var(--off);color:var(--dark);border:2px solid var(--dark);border-radius:22px;transition:transform .18s cubic-bezier(.2,.7,.3,1),box-shadow .18s cubic-bezier(.2,.7,.3,1);box-shadow:6px 6px 0 var(--green)}
.winner:hover{transform:translate(-3px,-3px);box-shadow:9px 9px 0 var(--green)}
.winner--first{background:var(--mint);box-shadow:6px 6px 0 var(--yellow)}
.winner--first:hover{box-shadow:9px 9px 0 var(--yellow)}
.award--weekly .winner{box-shadow:6px 6px 0 var(--pink)}
.award--weekly .winner:hover{box-shadow:9px 9px 0 var(--pink)}
.award--weekly .winner--first{background:var(--yellow);box-shadow:6px 6px 0 var(--orange)}
.award--weekly .winner--first:hover{box-shadow:9px 9px 0 var(--orange)}
.award--monthly .winner{box-shadow:6px 6px 0 var(--violet)}
.award--monthly .winner:hover{box-shadow:9px 9px 0 var(--violet)}
.award--monthly .winner--first{background:var(--pink);box-shadow:6px 6px 0 var(--purple)}
.award--monthly .winner--first:hover{box-shadow:9px 9px 0 var(--purple)}
.badge{flex-shrink:0}
.badge img,.badge .placeholder{width:74px;height:74px;border-radius:50%;border:2px solid var(--dark);object-fit:cover;background:var(--green);display:block}
.badge .placeholder{display:grid;place-items:center;font-family:'Bricolage Grotesque',sans-serif;font-weight:800;font-size:1.9rem;color:var(--dark)}
.winner .body{display:flex;flex-direction:column;gap:4px;min-width:0}
.winner .stamp{align-self:flex-start;display:inline-block;background:var(--dark);color:var(--mint);padding:3px 10px;font-family:'Bricolage Grotesque',sans-serif;font-weight:700;font-size:.74rem;border-radius:999px;margin-bottom:4px}
.winner--first .stamp{background:var(--dark);color:var(--yellow)}
.award--weekly .winner--first .stamp{background:var(--dark);color:var(--pink)}
.award--monthly .winner--first .stamp{background:var(--dark);color:var(--violet)}
.winner .period{font-family:'Inter',sans-serif;font-weight:500;font-size:.82rem;color:var(--dark);opacity:.65;font-variant-numeric:tabular-nums;letter-spacing:0}
.winner .name{font-family:'Bricolage Grotesque',sans-serif;font-weight:800;font-size:clamp(1.4rem,2vw,1.95rem);color:var(--dark);line-height:1.05;overflow-wrap:anywhere;transition:color .15s}
.winner .name:hover{color:var(--dark);text-decoration:underline;text-decoration-thickness:3px;text-decoration-color:var(--pink);text-underline-offset:4px}
.winner .score{text-align:left;padding-left:8px;border-left:2px solid rgba(7,36,27,.18);min-width:92px}
.winner .score .num{display:block;font-family:'Bricolage Grotesque',sans-serif;font-weight:800;font-size:clamp(2rem,3vw,2.8rem);color:var(--dark);line-height:1;font-variant-numeric:tabular-nums}
.winner .score .unit{display:block;margin-top:4px;font-size:.78rem;font-weight:600;color:var(--dark);opacity:.6}
.winner--empty{padding:36px;text-align:left;font-family:'Bricolage Grotesque',sans-serif;font-weight:600;font-size:1.15rem;color:var(--mint);background:transparent;border:2px dashed rgba(208,251,203,.38);box-shadow:none;grid-template-columns:1fr}
.winner--empty:hover{transform:none;box-shadow:none}
.howto{padding:clamp(44px,7vw,96px) 0;border-top:2px solid rgba(208,251,203,.14);position:relative}
.howto h2{color:var(--off);margin:8px 0 32px;font-size:clamp(2rem,3.2vw,3.2rem);max-width:22ch}
.howto ol{list-style:none;display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:20px}
.howto li{padding:30px;background:var(--off);color:var(--dark);border:2px solid var(--dark);border-radius:22px;position:relative;box-shadow:6px 6px 0 var(--dark)}
.howto li:nth-child(1){box-shadow:6px 6px 0 var(--green)}
.howto li:nth-child(2){box-shadow:6px 6px 0 var(--pink)}
.howto li:nth-child(3){box-shadow:6px 6px 0 var(--yellow)}
.howto .n{font-family:'Bricolage Grotesque',sans-serif;font-weight:800;font-size:3.4rem;line-height:.85;color:var(--green);display:block;margin-bottom:14px}
.howto li:nth-child(2) .n{color:var(--pink)}
.howto li:nth-child(3) .n{color:var(--orange)}
.howto li p{font-size:1.02rem;line-height:1.55;color:var(--dark)}
.foot{padding:52px 0 72px;border-top:2px solid rgba(208,251,203,.14);display:grid;grid-template-columns:1fr auto;gap:22px;align-items:center;color:var(--mint);font-size:.98rem}
.foot p{max-width:64ch}
.foot a{color:var(--off);border-bottom:1.5px solid rgba(249,247,246,.35);padding-bottom:1px;transition:color .15s,border-color .15s}
.foot a:hover{color:var(--green);border-bottom-color:var(--green)}
.foot nav{display:flex;gap:22px;flex-wrap:wrap}
@keyframes rise{from{opacity:0;transform:translateY(14px)}to{opacity:1;transform:none}}
.topbar,.hero,.award,.howto,.foot{animation:rise .7s cubic-bezier(.22,.7,.25,1) both}
.hero{animation-delay:.05s}
.award:nth-of-type(1){animation-delay:.1s}
.award:nth-of-type(2){animation-delay:.16s}
.award:nth-of-type(3){animation-delay:.22s}
.howto{animation-delay:.28s}
.foot{animation-delay:.34s}
@media (max-width:760px){
  .winner{grid-template-columns:auto 1fr;grid-template-rows:auto auto;gap:14px;padding:18px}
  .winner .score{grid-column:1/-1;border-left:0;border-top:1.5px dashed rgba(7,36,27,.25);padding:10px 0 0;min-width:0}
  .winner .score .num{font-size:2.1rem}
  .topbar{padding:18px 0}
  .hero{padding:24px 0 60px}
  .foot{grid-template-columns:1fr}
}
@media (prefers-reduced-motion:reduce){
  .status .dot,.topbar,.hero,.award,.howto,.foot,.winner{animation:none;transition:none}
  .winner:hover{transform:none}
}"#
}
