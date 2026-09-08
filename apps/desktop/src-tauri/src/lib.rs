#[cfg(windows)]
pub mod native;

pub fn allowed_navigation(url: &url::Url, development: bool) -> bool {
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    matches!(
        (url.scheme(), url.host_str(), url.port()),
        ("http", Some("tauri.localhost"), None) | ("tauri", Some("localhost"), None)
    ) || (development
        && url.scheme() == "http"
        && url.host_str() == Some("127.0.0.1")
        && url.port() == Some(1420))
}

#[cfg(test)]
mod tests {
    #[test]
    fn navigation_cannot_take_the_privileged_window_to_an_external_origin() {
        for url in ["http://tauri.localhost/", "tauri://localhost/index.html"] {
            assert!(super::allowed_navigation(&url.parse().unwrap(), false));
        }
        for url in [
            "https://example.com/",
            "file:///C:/data.txt",
            "http://tauri.localhost.evil/",
            "http://user@tauri.localhost/",
            "http://127.0.0.1:1420/",
        ] {
            assert!(!super::allowed_navigation(&url.parse().unwrap(), false));
        }
        assert!(super::allowed_navigation(
            &"http://127.0.0.1:1420/".parse().unwrap(),
            true
        ));
        assert!(!super::allowed_navigation(
            &"http://127.0.0.1:1421/".parse().unwrap(),
            true
        ));
    }
}
