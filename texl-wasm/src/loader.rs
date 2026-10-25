//! Document loaders of the web engine.
//!
//! `http://` and `https://` URLs are fetched as they are. Every document has a full URL
//! (the URIs of the host resolve against the base URL), and the engine resolves the imports
//! of a document to full URLs, absolute paths included: `/p/t/d` imported by
//! `http://host.com/path/to/doc` is `http://host.com/p/t/d`. So there is no default loader.

use url::Url;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    /// The global `fetch`, available both in windows and in workers
    #[wasm_bindgen(js_name = fetch, catch)]
    fn fetch(url: &str) -> Result<js_sys::Promise, JsValue>;

    type Response;

    #[wasm_bindgen(method, getter)]
    fn ok(this: &Response) -> bool;

    #[wasm_bindgen(method, getter)]
    fn status(this: &Response) -> u16;

    #[wasm_bindgen(method, catch)]
    fn text(this: &Response) -> Result<js_sys::Promise, JsValue>;
}

/// Fetches the document at the absolute `url`. Non-2xx responses are errors.
pub async fn load_http(url: String) -> Result<String, String> {
    let fail = |err: JsValue| format!("{url}: {}", err.as_string().unwrap_or_else(|| format!("{err:?}")));

    let response: Response = JsFuture::from(fetch(&url).map_err(fail)?)
        .await
        .map_err(fail)?
        .unchecked_into();
    if !response.ok() {
        return Err(format!("{url}: HTTP status {}", response.status()));
    }
    let text = JsFuture::from(response.text().map_err(fail)?).await.map_err(fail)?;
    text.as_string().ok_or_else(|| format!("{url}: the response is not text"))
}

/// Resolves the URI of a document loaded by the host against `base` (the page URL, by default).
pub fn resolve_document_url(uri: &str, base: Option<&Url>) -> Result<String, String> {
    match base {
        Some(base) => base.join(uri).map(String::from).map_err(|e| format!("{uri}: {e}")),
        None => Url::parse(uri).map(String::from).map_err(|e| format!("{uri}: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn test_resolve_document_url_resolves_relative_uris_against_the_base() {
        let base = url("http://host.com/app/index.html");
        assert_eq!(
            resolve_document_url("docs/main.texl", Some(&base)),
            Ok("http://host.com/app/docs/main.texl".into())
        );
        assert_eq!(resolve_document_url("/main.texl", Some(&base)), Ok("http://host.com/main.texl".into()));
        assert_eq!(
            resolve_document_url("https://other.com/main.texl", Some(&base)),
            Ok("https://other.com/main.texl".into())
        );
    }

    #[test]
    fn test_resolve_document_url_without_base_requires_absolute_uris() {
        assert_eq!(resolve_document_url("http://host.com/a", None), Ok("http://host.com/a".into()));
        assert!(resolve_document_url("docs/main.texl", None).is_err());
    }
}
