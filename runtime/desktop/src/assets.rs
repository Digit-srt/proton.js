use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use embedder_traits::WebResourceResponse;
use http::header::{CONTENT_TYPE, HeaderValue, X_CONTENT_TYPE_OPTIONS};
use http::{Method, StatusCode};
use percent_encoding::percent_decode_str;
use servo::{ServoDelegate, WebResourceLoad};
use url::Url;

const APP_HOST: &str = "app.proton.invalid";
pub const START_URL: &str = "https://app.proton.invalid/index.html";
const WELCOME: &[u8] = include_bytes!("../welcome.html");

pub struct AppAssets {
    root: Option<PathBuf>,
}

impl AppAssets {
    pub fn beside_executable() -> io::Result<Self> {
        let mut root = std::env::current_exe()?;
        root.pop();
        root.push("web");
        let root = root
            .join("index.html")
            .is_file()
            .then(|| root.canonicalize())
            .transpose()?;
        Ok(Self { root })
    }

    pub fn respond(&self, load: WebResourceLoad) {
        let request = load.request();
        let url = request.url.as_url();
        if url.scheme() != "https" || url.host_str() != Some(APP_HOST) || url.port().is_some() {
            return;
        }

        let mut response = WebResourceResponse::new(url.clone());
        response
            .headers
            .insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
        response.headers.insert(
            "cross-origin-resource-policy",
            HeaderValue::from_static("same-origin"),
        );
        let send_body = request.method == Method::GET;
        let mut body = None;
        if !send_body && request.method != Method::HEAD {
            response.status_code = StatusCode::METHOD_NOT_ALLOWED;
        } else if let Some((mime, bytes)) = self.read(url) {
            response.headers.insert(CONTENT_TYPE, mime);
            body = send_body.then_some(bytes);
        } else {
            response.status_code = StatusCode::NOT_FOUND;
        }
        if let Some(reason) = response.status_code.canonical_reason() {
            response.status_message = reason.as_bytes().to_vec();
        }

        let mut intercepted = load.intercept(response);
        if let Some(bytes) = body {
            intercepted.send_body_data(bytes);
        }
        intercepted.finish();
    }

    fn read(&self, url: &Url) -> Option<(HeaderValue, Vec<u8>)> {
        let mut relative = PathBuf::new();
        for encoded in url.path_segments()?.filter(|segment| !segment.is_empty()) {
            let segment = percent_decode_str(encoded).decode_utf8().ok()?;
            if segment.contains(['/', '\\', ':', '\0'])
                || !matches!(
                    Path::new(segment.as_ref()).components().next(),
                    Some(Component::Normal(_))
                )
            {
                return None;
            }
            relative.push(segment.as_ref());
        }
        if url.path().ends_with('/') {
            relative.push("index.html");
        }

        let Some(root) = &self.root else {
            return (relative == Path::new("index.html")).then(|| {
                (
                    HeaderValue::from_static("text/html; charset=utf-8"),
                    WELCOME.to_vec(),
                )
            });
        };
        let file = root.join(relative).canonicalize().ok()?;
        if !file.starts_with(root) || !file.is_file() {
            return None;
        }

        let mime = mime_guess::from_path(&file).first_or_octet_stream();
        let mime = HeaderValue::from_str(mime.essence_str()).ok()?;
        Some((mime, fs::read(file).ok()?))
    }
}

impl ServoDelegate for AppAssets {
    fn load_web_resource(&self, load: WebResourceLoad) {
        self.respond(load);
    }
}
