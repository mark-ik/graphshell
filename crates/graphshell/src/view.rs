use std::fmt::Write;

use base64::Engine;
use graphshell_client::{ResolvedContent, ResolvedPresentation};
use graphshell_protocol::{AdvertisedAction, SemanticRole};

use crate::canary::{CanaryError, CanaryRun, run_loopback_canary};

pub fn render_g1_receipt() -> Result<String, CanaryError> {
    run_loopback_canary().map(|run| render_canary_html(&run))
}

pub fn render_canary_html(run: &CanaryRun) -> String {
    let mut html = String::from(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Graphshell G1 · loopback presentation proof</title>
<style>
:root{color-scheme:dark;--ink:#f4eedf;--muted:#9eacb4;--line:#314756;--deep:#09141d;--panel:#10212c;--panel2:#142a37;--gold:#d8a657;--coral:#e46d5c;--sea:#79a9be}*{box-sizing:border-box}body{margin:0;min-height:100vh;background:radial-gradient(circle at 12% 0%,#193448 0,transparent 38%),linear-gradient(150deg,#071019,#0b1720 50%,#0d1b24);color:var(--ink);font:15px/1.5 Inter,ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif}.shell{width:min(1180px,calc(100% - 40px));margin:0 auto;padding:54px 0 64px}header{display:grid;grid-template-columns:1fr auto;gap:24px;align-items:end;margin-bottom:28px}.eyebrow{margin:0 0 9px;color:var(--gold);font-size:12px;font-weight:800;letter-spacing:.16em;text-transform:uppercase}h1{margin:0;font-size:clamp(31px,5vw,56px);font-weight:720;letter-spacing:-.045em;line-height:1.02}.lede{max-width:680px;margin:15px 0 0;color:#b9c5ca;font-size:17px}.session{align-self:start;border:1px solid var(--line);border-radius:999px;padding:8px 13px;color:#b9c5ca;background:#0a1821;font:12px ui-monospace,SFMono-Regular,Consolas,monospace}.profiles{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:20px}.profile{min-width:0;border:1px solid var(--line);border-radius:22px;background:linear-gradient(180deg,rgba(20,42,55,.96),rgba(13,29,39,.96));box-shadow:0 18px 55px rgba(0,0,0,.25);overflow:hidden}.profile-head{display:flex;justify-content:space-between;gap:16px;align-items:start;padding:20px 22px;border-bottom:1px solid var(--line);background:rgba(8,20,28,.5)}.profile h2{margin:0;font-size:19px;letter-spacing:-.015em}.profile p{margin:4px 0 0;color:var(--muted);font-size:13px}.status{display:inline-flex;align-items:center;gap:7px;color:#b9d5c5;font-size:12px;white-space:nowrap}.status:before{content:"";width:8px;height:8px;border-radius:50%;background:#65bd85;box-shadow:0 0 0 4px rgba(101,189,133,.12)}.scene{display:grid;grid-template-columns:1fr;gap:14px;padding:18px;min-height:430px}.item{position:relative;min-height:176px;border:1px solid #38505f;border-radius:16px;background:var(--panel);overflow:hidden}.card{padding:21px}.card-top{display:flex;justify-content:space-between;gap:14px;align-items:start}.card-kicker{color:var(--sea);font-size:11px;font-weight:800;letter-spacing:.13em;text-transform:uppercase}.card h3{margin:4px 0 16px;font-size:22px;letter-spacing:-.025em}.badges{display:flex;flex-wrap:wrap;gap:7px}.badge{border:1px solid #415b6b;border-radius:999px;padding:4px 8px;color:#c4d2d8;background:#132a37;font-size:11px}dl{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px;margin:18px 0}dl div{border-left:2px solid #34566a;padding-left:10px}dt{color:var(--muted);font-size:11px;text-transform:uppercase;letter-spacing:.08em}dd{margin:3px 0 0;font-weight:650}.glyph{display:grid;grid-template-columns:64px 1fr;gap:15px;align-items:center;padding:24px}.glyph-mark{display:grid;place-items:center;width:64px;height:64px;border:1px solid #7a653e;border-radius:20px;background:radial-gradient(circle at 35% 30%,#4d4028,#201d19);color:var(--gold);font-size:32px}.glyph h3{margin:0;font-size:19px}.glyph p{margin:5px 0 0}.image{display:flex;flex-direction:column}.image img{display:block;width:100%;height:176px;object-fit:cover;background:#163044}.image-meta{display:flex;justify-content:space-between;gap:12px;align-items:center;padding:12px 14px}.placeholder{display:grid;place-items:center;text-align:center;padding:26px;border-style:dashed;background:repeating-linear-gradient(135deg,#10212c,#10212c 11px,#122633 11px,#122633 22px)}.placeholder-mark{font-size:32px;color:#718795}.placeholder h3{margin:7px 0 2px}.placeholder p{max-width:260px}.actions{display:flex;flex-wrap:wrap;gap:8px;margin-top:15px}button{appearance:none;border:1px solid #566f7e;border-radius:10px;padding:8px 11px;background:#1a3443;color:var(--ink);font:inherit;font-size:12px;font-weight:700;cursor:pointer}button:hover,button:focus-visible{border-color:var(--gold);outline:2px solid rgba(216,166,87,.3);outline-offset:2px}.proof{display:flex;flex-wrap:wrap;gap:9px;margin-top:22px}.proof span{border:1px solid #2c4351;border-radius:9px;padding:7px 10px;background:#0b1922;color:#9fb0b8;font-size:12px}.proof strong{color:#dbe5e8}@media(max-width:760px){.shell{width:min(100% - 24px,620px);padding-top:30px}header{grid-template-columns:1fr}.session{justify-self:start}.profiles{grid-template-columns:1fr}.scene{min-height:0}}
</style>
</head>
<body>
<main class="shell">
<header>
<div><p class="eyebrow">Graphshell · G1 receipt</p><h1>One scene,<br>two capabilities.</h1><p class="lede">A loopback endpoint discloses one Scenograph scene. Graphshell resolves its presentation resources locally, preserving labels and actions as richer codecs fall away.</p></div>
<div class="session">loopback:g1-presentation · rev 1</div>
</header>
<div class="profiles">
"##,
    );
    render_profile(
        &mut html,
        "Rich profile",
        "Card, glyph, and image codecs",
        &run.rich,
    );
    render_profile(
        &mut html,
        "Compact profile",
        "Native glyph codec only",
        &run.compact,
    );
    html.push_str(
        r##"</div>
<div class="proof" aria-label="Proof conditions"><span><strong>Scene:</strong> product-free</span><span><strong>Resources:</strong> content-addressed</span><span><strong>Cache:</strong> session-scoped</span><span><strong>Fallback:</strong> labeled</span><span><strong>Actions:</strong> keyboard reachable</span></div>
</main>
</body>
</html>
"##,
    );
    html
}

fn render_profile(
    html: &mut String,
    title: &str,
    subtitle: &str,
    presentations: &[ResolvedPresentation],
) {
    write!(
        html,
        "<section class=\"profile\" aria-label=\"{}\"><div class=\"profile-head\"><div><h2>{}</h2><p>{}</p></div><span class=\"status\">Live</span></div><div class=\"scene\">",
        escape(title),
        escape(title),
        escape(subtitle)
    )
    .unwrap();
    for presentation in presentations {
        render_item(html, presentation);
    }
    html.push_str("</div></section>\n");
}

fn render_item(html: &mut String, presentation: &ResolvedPresentation) {
    let role = semantic_role(presentation.semantics.role);
    match &presentation.content {
        ResolvedContent::PortableCard(card) => {
            write!(
                html,
                "<article class=\"item card\" aria-label=\"{}\"><div class=\"card-top\"><div><span class=\"card-kicker\">Portable card</span><h3>{}</h3></div><div class=\"badges\">",
                escape(&presentation.semantics.label),
                escape(&card.title)
            )
            .unwrap();
            for badge in &card.badges {
                write!(html, "<span class=\"badge\">{}</span>", escape(badge)).unwrap();
            }
            html.push_str("</div></div><dl>");
            for value in &card.values {
                write!(
                    html,
                    "<div><dt>{}</dt><dd>{}</dd></div>",
                    escape(&value.label),
                    escape(&value.value)
                )
                .unwrap();
            }
            html.push_str("</dl>");
            render_actions(html, &presentation.semantics.actions);
            html.push_str("</article>");
        }
        ResolvedContent::NativeGlyph(glyph) => {
            write!(
                html,
                "<div class=\"item glyph\" role=\"{}\" aria-label=\"{}\"><div class=\"glyph-mark\" aria-hidden=\"true\">{}</div><div><h3>{}</h3><p>Native Graphshell glyph</p>",
                role,
                escape(&presentation.semantics.label),
                escape(glyph.icon.as_deref().unwrap_or("•")),
                escape(&glyph.label)
            )
            .unwrap();
            render_actions(html, &presentation.semantics.actions);
            html.push_str("</div></div>");
        }
        ResolvedContent::Image { mime_type, bytes } => {
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            write!(
                html,
                "<div class=\"item image\" role=\"{}\" aria-label=\"{}\"><img alt=\"{}\" src=\"data:{};base64,{}\"><div class=\"image-meta\"><div><strong>{}</strong><p>Content-addressed image resource</p></div>",
                role,
                escape(&presentation.semantics.label),
                escape(&presentation.semantics.label),
                escape(mime_type),
                encoded,
                escape(&presentation.semantics.label)
            )
            .unwrap();
            render_actions(html, &presentation.semantics.actions);
            html.push_str("</div></div>");
        }
        ResolvedContent::LabeledPlaceholder => {
            write!(
                html,
                "<div class=\"item placeholder\" role=\"{}\" aria-label=\"{}\"><div><div class=\"placeholder-mark\" aria-hidden=\"true\">◇</div><h3>{}</h3><p>Image capability unavailable. The disclosed label and action remain.</p>",
                role,
                escape(&presentation.semantics.label),
                escape(&presentation.semantics.label)
            )
            .unwrap();
            render_actions(html, &presentation.semantics.actions);
            html.push_str("</div></div>");
        }
    }
}

fn render_actions(html: &mut String, actions: &[AdvertisedAction]) {
    if actions.is_empty() {
        return;
    }
    html.push_str("<div class=\"actions\">");
    for action in actions {
        write!(
            html,
            "<button type=\"button\" data-intent=\"{}\" title=\"{}\">{}</button>",
            escape(&action.intent.0),
            escape(&action.explanation),
            escape(&action.label)
        )
        .unwrap();
    }
    html.push_str("</div>");
}

fn semantic_role(role: SemanticRole) -> &'static str {
    match role {
        SemanticRole::Graphic => "img",
        SemanticRole::Article => "article",
        SemanticRole::Image => "img",
    }
}

fn escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_contains_both_resolutions_and_real_actions() {
        let html = render_g1_receipt().unwrap();
        assert!(html.contains("Portable card"));
        assert!(html.contains("Native Graphshell glyph"));
        assert!(html.contains("data:image/svg+xml;base64,"));
        assert!(html.contains("Image capability unavailable"));
        assert_eq!(html.matches("data-intent=\"fixture.open-note\"").count(), 2);
        assert_eq!(
            html.matches("data-intent=\"fixture.inspect-tile\"").count(),
            2
        );
    }

    #[test]
    fn committed_receipt_matches_the_live_loopback_view() {
        assert_eq!(
            render_g1_receipt().unwrap(),
            include_str!("../../../docs/receipts/g1_loopback.html")
        );
    }
}
