const TARGET_OS: &str = "CARGO_CFG_TARGET_OS";
const WINDOWS: &str = "windows";
const COMMON_CONTROLS: &str = "/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'";

/// Builds Tauri resources and gives every Windows executable, including unit-test harnesses, the subclassing API's v6 dependency.
fn main() {
    tauri_build::build();
    if std::env::var(TARGET_OS).as_deref() == Ok(WINDOWS) {
        println!("cargo:rustc-link-arg={COMMON_CONTROLS}");
    }
}
