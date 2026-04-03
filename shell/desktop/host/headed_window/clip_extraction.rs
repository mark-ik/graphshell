/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! JavaScript injection scripts and result parsers for clip/inspector extraction.

use euclid::Point2D;
use serde_json::Value as JsonValue;
use servo::{DeviceIndependentPixel, DeviceIntRect, JSValue, JavaScriptEvaluationError, WebViewId};

use crate::app::ClipCaptureData;

pub(super) fn build_clip_extraction_script(element_rect: DeviceIntRect) -> String {
    let center_x = (element_rect.min.x + element_rect.max.x) as f32 / 2.0;
    let center_y = (element_rect.min.y + element_rect.max.y) as f32 / 2.0;
    format!(
        r#"(function() {{
            const dpr = window.devicePixelRatio || 1;
            const x = {center_x} / dpr;
            const y = {center_y} / dpr;
            const element = document.elementFromPoint(x, y) || document.activeElement || document.body;
            if (!element) {{
                return JSON.stringify({{ ok: false, error: "No element found under context menu target." }});
            }}

            const titleFor = (target, tagName, textValue) =>
                (textValue ||
                    target.getAttribute("aria-label") ||
                    target.getAttribute("title") ||
                    target.getAttribute("alt") ||
                    target.querySelector?.("h1,h2,h3,h4,strong,b,figcaption,caption")?.textContent ||
                    (tagName ? `Clip: <${{tagName}}>` : "Clipped element"));
            const textValue = (element.innerText || element.textContent || "")
                .replace(/\s+/g, " ")
                .trim();
            const tagName = (element.tagName || "").toLowerCase();
            const titleValue = titleFor(element, tagName, textValue);
            const link = element.closest ? element.closest("a") : null;
            const image =
                tagName === "img"
                    ? element
                    : (element.querySelector ? element.querySelector("img,picture source,video,canvas,svg") : null);
            const domPathFor = (target) => {{
                if (!(target instanceof Element)) return null;
                const segments = [];
                let current = target;
                while (current && current.nodeType === 1 && current !== document.body) {{
                    const tag = (current.tagName || "div").toLowerCase();
                    let index = 1;
                    let sibling = current;
                    while ((sibling = sibling.previousElementSibling)) {{
                        if ((sibling.tagName || "").toLowerCase() === tag) {{
                            index += 1;
                        }}
                    }}
                    segments.unshift(`${{tag}}:nth-of-type(${{index}})`);
                    current = current.parentElement;
                }}
                segments.unshift("body");
                return segments.join(" > ");
            }};

            return JSON.stringify({{
                ok: true,
                source_url: window.location.href,
                page_title: document.title || null,
                clip_title: titleValue,
                outer_html: element.outerHTML || "",
                text_excerpt: textValue,
                tag_name: tagName,
                href: (link && link.href) || element.getAttribute("href") || null,
                image_url: (image && image.src) || null,
                dom_path: domPathFor(element)
            }});
        }})()"#
    )
}

pub(super) fn build_page_inspector_extraction_script() -> String {
    r#"(function() {
        const normalizeWhitespace = (value) =>
            (value || "").replace(/\s+/g, " ").trim();
        const titleFor = (element, tagName, textValue) =>
            normalizeWhitespace(
                textValue ||
                element.getAttribute("aria-label") ||
                element.getAttribute("title") ||
                element.getAttribute("alt") ||
                element.querySelector?.("h1,h2,h3,h4,strong,b,figcaption,caption")?.textContent ||
                (tagName ? `Clip: <${tagName}>` : "Clipped element")
            );
        const domPathFor = (element) => {
            if (!(element instanceof Element)) return null;
            const segments = [];
            let current = element;
            while (current && current.nodeType === 1 && current !== document.body) {
                const tag = (current.tagName || "div").toLowerCase();
                let index = 1;
                let sibling = current;
                while ((sibling = sibling.previousElementSibling)) {
                    if ((sibling.tagName || "").toLowerCase() === tag) {
                        index += 1;
                    }
                }
                segments.unshift(`${tag}:nth-of-type(${index})`);
                current = current.parentElement;
            }
            segments.unshift("body");
            return segments.join(" > ");
        };
        const toPayload = (element) => {
            const tagName = (element.tagName || "").toLowerCase();
            const textValue = normalizeWhitespace(element.innerText || element.textContent || "");
            const link = element.closest ? element.closest("a") : null;
            const image =
                tagName === "img"
                    ? element
                    : (element.querySelector ? element.querySelector("img,picture source,video,canvas,svg") : null);
            return {
                source_url: window.location.href,
                page_title: document.title || null,
                clip_title: titleFor(element, tagName, textValue),
                outer_html: element.outerHTML || "",
                text_excerpt: textValue,
                tag_name: tagName,
                href: (link && link.href) || element.getAttribute("href") || null,
                image_url: (image && image.src) || null,
                dom_path: domPathFor(element)
            };
        };

        const candidates = Array.from(
            document.querySelectorAll(
                "main article, article, section, aside, figure, nav, header, footer, picture, video, audio, svg, canvas, img, table, pre, blockquote, h1, h2, h3, li"
            )
        );

        const seen = new Set();
        const clips = [];
        for (const element of candidates) {
            if (!element || seen.has(element)) {
                continue;
            }
            const rect = element.getBoundingClientRect();
            if (rect.width < 120 || rect.height < 36) {
                continue;
            }
            const textValue = normalizeWhitespace(element.innerText || element.textContent || "");
            const score =
                Math.min(textValue.length, 280) +
                Math.min(rect.width * rect.height / 1800, 180) +
                (element.querySelector?.("img,picture,video,canvas,svg") ? 60 : 0) +
                (/^(article|section|figure|aside|nav|header|footer|table|blockquote|pre)$/i.test(element.tagName || "") ? 45 : 0) +
                (/^(picture|video|audio|svg|canvas|img)$/i.test(element.tagName || "") ? 55 : 0) +
                (/^h[1-3]$/i.test(element.tagName || "") ? 35 : 0);
            clips.push({ element, score, rectTop: rect.top });
            seen.add(element);
        }

        clips.sort((a, b) => b.score - a.score || a.rectTop - b.rectTop);
        const selected = [];
        const selectedTitles = new Set();
        for (const candidate of clips) {
            const payload = toPayload(candidate.element);
            if (!payload.outer_html || payload.outer_html.length > 24000) {
                continue;
            }
            if (
                !payload.text_excerpt &&
                !payload.image_url &&
                !payload.href &&
                !/^(picture|video|audio|svg|canvas|figure|img)$/i.test(payload.tag_name || "")
            ) {
                continue;
            }
            const titleKey = (payload.dom_path || payload.clip_title).toLowerCase();
            if (selectedTitles.has(titleKey)) {
                continue;
            }
            selected.push(payload);
            selectedTitles.add(titleKey);
            if (selected.length >= 8) {
                break;
            }
        }

        if (selected.length === 0) {
            return JSON.stringify({ ok: false, error: "No salient page components were found to clip." });
        }

        return JSON.stringify({ ok: true, clips: selected });
    })()"#
        .to_string()
}

pub(super) fn build_clip_inspector_stack_script(
    local_point: Point2D<f32, DeviceIndependentPixel>,
) -> String {
    let x = local_point.x;
    let y = local_point.y;
    format!(
        r#"(function() {{
            const normalizeWhitespace = (value) => (value || "").replace(/\s+/g, " ").trim();
            const domPathFor = (element) => {{
                if (!(element instanceof Element)) return null;
                const segments = [];
                let current = element;
                while (current && current.nodeType === 1 && current !== document.body) {{
                    const tag = (current.tagName || "div").toLowerCase();
                    let index = 1;
                    let sibling = current;
                    while ((sibling = sibling.previousElementSibling)) {{
                        if ((sibling.tagName || "").toLowerCase() === tag) {{
                            index += 1;
                        }}
                    }}
                    segments.unshift(`${{tag}}:nth-of-type(${{index}})`);
                    current = current.parentElement;
                }}
                segments.unshift("body");
                return segments.join(" > ");
            }};
            const toPayload = (element) => {{
                const tagName = (element.tagName || "").toLowerCase();
                const textValue = normalizeWhitespace(element.innerText || element.textContent || "");
                const link = element.closest ? element.closest("a") : null;
                const image =
                    tagName === "img"
                        ? element
                        : (element.querySelector ? element.querySelector("img,picture source,video,canvas,svg") : null);
                return {{
                    source_url: window.location.href,
                    page_title: document.title || null,
                    clip_title:
                        textValue ||
                        element.getAttribute("aria-label") ||
                        element.getAttribute("title") ||
                        element.getAttribute("alt") ||
                        element.querySelector?.("h1,h2,h3,h4,strong,b,figcaption,caption")?.textContent ||
                        (tagName ? `Clip: <${{tagName}}>` : "Clipped element"),
                    outer_html: element.outerHTML || "",
                    text_excerpt: textValue,
                    tag_name: tagName,
                    href: (link && link.href) || element.getAttribute("href") || null,
                    image_url: (image && image.src) || null,
                    dom_path: domPathFor(element)
                }};
            }};

            const dpr = window.devicePixelRatio || 1;
            const stack = (document.elementsFromPoint({x} / dpr, {y} / dpr) || [])
                .filter((element) => element instanceof Element)
                .filter((element, index, arr) => arr.indexOf(element) === index)
                .slice(0, 8)
                .map(toPayload);
            if (stack.length === 0) {{
                return JSON.stringify({{ ok: false, error: "No DOM elements found under pointer." }});
            }}
            return JSON.stringify({{ ok: true, clips: stack }});
        }})()"#
    )
}

pub(super) fn build_clip_inspector_highlight_script(dom_path: Option<&str>) -> String {
    let dom_path_json = serde_json::to_string(&dom_path).unwrap_or_else(|_| "null".to_string());
    format!(
        r##"(function() {{
            const OVERLAY_ID = "__graphshell_clip_inspector_overlay__";
            const LABEL_ID = "__graphshell_clip_inspector_label__";
            const existingOverlay = document.getElementById(OVERLAY_ID);
            const existingLabel = document.getElementById(LABEL_ID);
            if (existingOverlay) existingOverlay.remove();
            if (existingLabel) existingLabel.remove();

            const selector = {dom_path_json};
            if (!selector) {{
                return JSON.stringify({{ ok: true }});
            }}

            const element = document.querySelector(selector);
            if (!element) {{
                return JSON.stringify({{ ok: false, error: "Inspector highlight target not found." }});
            }}

            const rect = element.getBoundingClientRect();
            const overlay = document.createElement("div");
            overlay.id = OVERLAY_ID;
            overlay.style.position = "fixed";
            overlay.style.left = `${{rect.left}}px`;
            overlay.style.top = `${{rect.top}}px`;
            overlay.style.width = `${{rect.width}}px`;
            overlay.style.height = `${{rect.height}}px`;
            overlay.style.border = "2px solid #ff7a00";
            overlay.style.background = "rgba(255, 122, 0, 0.12)";
            overlay.style.boxShadow = "0 0 0 9999px rgba(14, 10, 6, 0.12)";
            overlay.style.pointerEvents = "none";
            overlay.style.zIndex = "2147483646";
            overlay.style.borderRadius = "8px";
            document.documentElement.appendChild(overlay);

            const label = document.createElement("div");
            label.id = LABEL_ID;
            label.textContent = selector;
            label.style.position = "fixed";
            label.style.left = `${{Math.max(rect.left, 8)}}px`;
            label.style.top = `${{Math.max(rect.top - 28, 8)}}px`;
            label.style.padding = "4px 8px";
            label.style.background = "#1f1610";
            label.style.color = "#fff8f0";
            label.style.font = "12px monospace";
            label.style.borderRadius = "999px";
            label.style.pointerEvents = "none";
            label.style.zIndex = "2147483647";
            document.documentElement.appendChild(label);

            return JSON.stringify({{ ok: true }});
        }})()"##
    )
}

pub(super) fn parse_clip_capture_result(
    webview_id: WebViewId,
    result: Result<JSValue, JavaScriptEvaluationError>,
) -> Result<ClipCaptureData, String> {
    let value = parse_clip_json_result(result)?;
    clip_capture_data_from_value(webview_id, &value)
}

pub(super) fn parse_clip_capture_batch_result(
    webview_id: WebViewId,
    result: Result<JSValue, JavaScriptEvaluationError>,
) -> Result<Vec<ClipCaptureData>, String> {
    let value = parse_clip_json_result(result)?;
    let clips = value
        .get("clips")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "clip payload missing required field `clips`".to_string())?;
    let mut captures = Vec::with_capacity(clips.len());
    for clip in clips {
        captures.push(clip_capture_data_from_value(webview_id, clip)?);
    }
    if captures.is_empty() {
        return Err("clip payload contained no captured clips".to_string());
    }
    Ok(captures)
}

fn parse_clip_json_result(
    result: Result<JSValue, JavaScriptEvaluationError>,
) -> Result<JsonValue, String> {
    let json = match result {
        Ok(JSValue::String(json)) => json,
        Ok(other) => return Err(format!("unexpected JavaScript clip result: {other:?}")),
        Err(error) => return Err(format!("JavaScript clip extraction failed: {error:?}")),
    };

    let value: JsonValue = serde_json::from_str(&json)
        .map_err(|error| format!("invalid clip payload JSON: {error}"))?;
    if value.get("ok").and_then(JsonValue::as_bool) != Some(true) {
        let detail = value
            .get("error")
            .and_then(JsonValue::as_str)
            .unwrap_or("Clip extraction returned an unknown failure.");
        return Err(detail.to_string());
    }
    Ok(value)
}

fn clip_capture_data_from_value(
    webview_id: WebViewId,
    value: &JsonValue,
) -> Result<ClipCaptureData, String> {
    let required = |key: &str| -> Result<String, String> {
        value
            .get(key)
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
            .filter(|entry| !entry.is_empty())
            .ok_or_else(|| format!("clip payload missing required field `{key}`"))
    };

    Ok(ClipCaptureData {
        webview_id,
        source_url: required("source_url")?,
        page_title: value
            .get("page_title")
            .and_then(JsonValue::as_str)
            .map(str::to_owned),
        clip_title: required("clip_title")?,
        outer_html: required("outer_html")?,
        text_excerpt: value
            .get("text_excerpt")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_string(),
        tag_name: value
            .get("tag_name")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_string(),
        href: value
            .get("href")
            .and_then(JsonValue::as_str)
            .map(str::to_owned),
        image_url: value
            .get("image_url")
            .and_then(JsonValue::as_str)
            .map(str::to_owned),
        dom_path: value
            .get("dom_path")
            .and_then(JsonValue::as_str)
            .map(str::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_clip_extraction_script, build_clip_inspector_stack_script,
        build_page_inspector_extraction_script,
    };
    use euclid::Point2D;
    use servo::{DeviceIndependentPixel, DeviceIntRect, DeviceIntSize};

    #[test]
    fn page_inspector_script_includes_broadened_candidate_selectors() {
        let script = build_page_inspector_extraction_script();

        assert!(script.contains("nav, header, footer, picture, video, audio, svg, canvas"));
        assert!(script.contains("table, pre, blockquote"));
    }

    #[test]
    fn page_inspector_script_prefers_dom_path_for_dedup_keys() {
        let script = build_page_inspector_extraction_script();

        assert!(script.contains("const titleKey = (payload.dom_path || payload.clip_title).toLowerCase();"));
        assert!(script.contains("figcaption,caption"));
    }

    #[test]
    fn extraction_scripts_cover_richer_media_targets() {
        let single = build_clip_extraction_script(DeviceIntRect::from_origin_and_size(
            euclid::point2(0, 0),
            DeviceIntSize::new(120, 60),
        ));
        let stack = build_clip_inspector_stack_script(Point2D::<f32, DeviceIndependentPixel>::new(
            24.0, 48.0,
        ));

        assert!(single.contains("img,picture source,video,canvas,svg"));
        assert!(stack.contains("img,picture source,video,canvas,svg"));
        assert!(stack.contains("figcaption,caption"));
    }
}
