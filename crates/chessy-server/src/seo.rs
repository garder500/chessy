//! SEO: serves the client's `index.html` with the page metadata filled in
//! (title, description, Open Graph, Twitter Card, canonical), plus
//! `robots.txt` and `sitemap.xml`. Crawlers and link previews (Discord,
//! Twitter, ...) do not run the client's JavaScript, so this is done here.
//!
//! The template carries `{{NAME}}` markers; see `web/index.html`.

use std::path::Path;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

/// Supported interface languages, as in the client; `en` is the fallback.
const LANGS: [&str; 5] = ["en", "fr", "de", "es", "pt"];

struct Meta {
    og_locale: &'static str,
    title: &'static str,
    description: &'static str,
    image_alt: &'static str,
}

fn meta(lang: &str) -> Meta {
    match lang {
        "fr" => Meta {
            og_locale: "fr_FR",
            title: "Chessy : les échecs en ligne avec des compétences",
            description: "Jouez aux échecs en ligne avec des compétences qui changent les règles : parties classées, mode solo contre l'IA, replays et analyse. Gratuit, sans installation.",
            image_alt: "Chessy, les échecs version compétences : un roi blanc sur fond graphite et jade.",
        },
        "de" => Meta {
            og_locale: "de_DE",
            title: "Chessy: Online-Schach mit Fähigkeiten",
            description: "Spiele Online-Schach mit Fähigkeiten, die die Regeln verändern: Ranglistenpartien, Solomodus gegen die KI, Replays und Analyse. Kostenlos, ohne Installation.",
            image_alt: "Chessy, Schach mit Fähigkeiten: ein weißer König auf Graphit und Jade.",
        },
        "es" => Meta {
            og_locale: "es_ES",
            title: "Chessy: ajedrez en línea con habilidades",
            description: "Juega al ajedrez en línea con habilidades que cambian las reglas: partidas clasificatorias, modo solitario contra la IA, repeticiones y análisis. Gratis, sin instalación.",
            image_alt: "Chessy, ajedrez con habilidades: un rey blanco sobre grafito y jade.",
        },
        "pt" => Meta {
            og_locale: "pt_PT",
            title: "Chessy: xadrez online com habilidades",
            description: "Jogue xadrez online com habilidades que mudam as regras: partidas classificativas, modo solo contra a IA, replays e análise. Grátis, sem instalação.",
            image_alt: "Chessy, xadrez com habilidades: um rei branco sobre grafite e jade.",
        },
        _ => Meta {
            og_locale: "en_US",
            title: "Chessy: online chess with skills",
            description: "Play online chess with skills that bend the rules: ranked games, solo mode against the AI, replays and analysis. Free, no install needed.",
            image_alt: "Chessy, chess with skills: a white king on a graphite and jade background.",
        },
    }
}

/// First supported language of an `Accept-Language` header. No header at all
/// (link-preview bots send none) gives French, the site's original language;
/// a header with no supported language gives English.
pub fn pick_lang(accept_language: Option<&str>) -> &'static str {
    let Some(header) = accept_language else {
        return "fr";
    };
    let mut ranked: Vec<(f32, usize, &str)> = header
        .split(',')
        .enumerate()
        .filter_map(|(i, part)| {
            let mut it = part.split(';');
            let tag = it.next()?.trim();
            let q = it
                .find_map(|p| {
                    p.trim()
                        .strip_prefix("q=")
                        .map(|v| v.trim().parse().unwrap_or(0.0))
                })
                .unwrap_or(1.0f32);
            (q > 0.0).then_some((q, i, tag))
        })
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    for (_, _, tag) in ranked {
        let base = tag
            .split(['-', '_'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if let Some(l) = LANGS.iter().find(|l| **l == base) {
            return l;
        }
    }
    "en"
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Fills the template for `lang` and the public origin `site_url` (no trailing slash).
pub fn render_index(template: &str, site_url: &str, lang: &str) -> String {
    let m = meta(lang);
    let lang = if LANGS.contains(&lang) { lang } else { "en" };
    [
        ("{{LANG}}", lang),
        ("{{OG_LOCALE}}", m.og_locale),
        ("{{TITLE}}", m.title),
        ("{{DESCRIPTION}}", m.description),
        ("{{IMAGE_ALT}}", m.image_alt),
        ("{{SITE_URL}}", site_url),
    ]
    .iter()
    .fold(template.to_string(), |html, (k, v)| {
        html.replace(k, &escape(v))
    })
}

/// The public origin: `CHESSY_PUBLIC_URL` when set, else built from the request
/// (`X-Forwarded-Proto` and `Host`), accepting only a plain host name.
fn site_url(configured: &Option<String>, headers: &HeaderMap) -> String {
    if let Some(url) = configured {
        return url.clone();
    }
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .filter(|h| {
            !h.is_empty()
                && h.len() <= 255
                && h.bytes().all(|b| {
                    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':' | b'[' | b']')
                })
        })
        .unwrap_or("localhost");
    let proto = match headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
    {
        Some("https") => "https",
        _ => "http",
    };
    format!("{proto}://{host}")
}

struct Seo {
    template: String,
    public_url: Option<String>,
}

type Shared = Arc<Seo>;

async fn index(State(seo): State<Shared>, headers: HeaderMap) -> Response {
    let lang = pick_lang(
        headers
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|h| h.to_str().ok()),
    );
    let html = render_index(&seo.template, &site_url(&seo.public_url, &headers), lang);
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::VARY, "Accept-Language, Host"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        html,
    )
        .into_response()
}

/// Same page for unknown paths, keeping the 404 status the static fallback had.
async fn index_not_found(state: State<Shared>, headers: HeaderMap) -> Response {
    let mut res = index(state, headers).await;
    *res.status_mut() = StatusCode::NOT_FOUND;
    res
}

async fn robots(State(seo): State<Shared>, headers: HeaderMap) -> Response {
    let base = site_url(&seo.public_url, &headers);
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        format!("User-agent: *\nAllow: /\nDisallow: /api/\nDisallow: /ws\n\nSitemap: {base}/sitemap.xml\n"),
    )
        .into_response()
}

async fn sitemap(State(seo): State<Shared>, headers: HeaderMap) -> Response {
    let base = escape(&site_url(&seo.public_url, &headers));
    (
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n  <url><loc>{base}/</loc></url>\n</urlset>\n"
        ),
    )
        .into_response()
}

/// Routes for the SEO pages, and the service for unknown paths. `None` when
/// `web_dir` holds no built client.
pub fn routes(web_dir: &Path) -> Option<(Router, Router)> {
    let template = std::fs::read_to_string(web_dir.join("index.html")).ok()?;
    let public_url = std::env::var("CHESSY_PUBLIC_URL")
        .ok()
        .map(|u| u.trim().trim_end_matches('/').to_string())
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"));
    let seo: Shared = Arc::new(Seo {
        template,
        public_url,
    });
    let pages = Router::new()
        .route("/", get(index))
        .route("/index.html", get(index))
        .route("/robots.txt", get(robots))
        .route("/sitemap.xml", get(sitemap))
        .with_state(seo.clone());
    let not_found = Router::new().fallback(index_not_found).with_state(seo);
    Some((pages, not_found))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_negotiation() {
        assert_eq!(pick_lang(None), "fr");
        assert_eq!(pick_lang(Some("fr-CA,fr;q=0.9,en;q=0.8")), "fr");
        assert_eq!(pick_lang(Some("ja,de;q=0.5,en;q=0.9")), "en");
        assert_eq!(pick_lang(Some("de;q=0.4, pt-BR;q=0.8")), "pt");
        assert_eq!(pick_lang(Some("ja, zh")), "en");
        assert_eq!(pick_lang(Some("fr;q=0, es")), "es");
    }

    #[test]
    fn template_is_filled_and_escaped() {
        let t = "<html lang=\"{{LANG}}\"><title>{{TITLE}}</title><meta content=\"{{SITE_URL}}\">";
        let out = render_index(t, "https://x.test\"><b>", "es");
        assert!(out.contains("lang=\"es\""));
        assert!(out.contains("ajedrez"));
        assert!(out.contains("https://x.test&quot;&gt;&lt;b&gt;"));
        assert!(!out.contains("{{"));
    }

    #[tokio::test]
    async fn sitemap_uses_the_standard_namespace() {
        let seo: Shared = Arc::new(Seo {
            template: String::new(),
            public_url: Some("https://c.test".into()),
        });
        let res = sitemap(State(seo), HeaderMap::new()).await;
        let body = axum::body::to_bytes(res.into_body(), 4096).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\""));
        assert!(body.contains("<loc>https://c.test/</loc>"));
    }

    #[test]
    fn host_header_is_validated() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, "chessy.example".parse().unwrap());
        h.insert("x-forwarded-proto", "https".parse().unwrap());
        assert_eq!(site_url(&None, &h), "https://chessy.example");
        h.insert(header::HOST, "a\"b".parse().unwrap());
        assert_eq!(site_url(&None, &h), "https://localhost");
        assert_eq!(
            site_url(&Some("https://c.test".into()), &h),
            "https://c.test"
        );
    }
}
