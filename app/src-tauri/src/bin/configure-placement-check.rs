//! Isolated real-WebView Configure regression; no input hooks or user library files are used.

/// Runs the bounded placement, focus, draft-retention, and native recovery observer check against Vite.
fn main() {
    macroloom_lib::check_configure_placement();
}
