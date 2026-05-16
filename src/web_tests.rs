use super::*;

#[test]
fn web_assets_available_is_callable() {
    let _ = web_assets_available();
}

#[test]
fn guess_mime_html() {
    assert_eq!(guess_mime("index.html"), "text/html; charset=utf-8");
}

#[test]
fn guess_mime_css() {
    assert_eq!(guess_mime("styles.css"), "text/css; charset=utf-8");
}

#[test]
fn guess_mime_js() {
    assert_eq!(
        guess_mime("app.js"),
        "application/javascript; charset=utf-8"
    );
}

#[test]
fn guess_mime_mjs() {
    assert_eq!(
        guess_mime("module.mjs"),
        "application/javascript; charset=utf-8"
    );
}

#[test]
fn guess_mime_json() {
    assert_eq!(guess_mime("data.json"), "application/json");
}

#[test]
fn guess_mime_svg() {
    assert_eq!(guess_mime("icon.svg"), "image/svg+xml");
}

#[test]
fn guess_mime_png() {
    assert_eq!(guess_mime("logo.png"), "image/png");
}

#[test]
fn guess_mime_ico() {
    assert_eq!(guess_mime("favicon.ico"), "image/x-icon");
}

#[test]
fn guess_mime_woff2() {
    assert_eq!(guess_mime("font.woff2"), "font/woff2");
}

#[test]
fn guess_mime_webmanifest() {
    assert_eq!(guess_mime("site.webmanifest"), "application/manifest+json");
}

#[test]
fn guess_mime_unknown_falls_back_to_octet_stream() {
    assert_eq!(guess_mime("archive.tar.bz2"), "application/octet-stream");
}

#[test]
fn guess_mime_no_extension_falls_back_to_octet_stream() {
    assert_eq!(guess_mime("Makefile"), "application/octet-stream");
}
