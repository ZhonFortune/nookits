fn main() {
    const REPOSITORY_ENV: &str = "NOOKIT_GITHUB_URL";
    println!("cargo:rerun-if-env-changed={REPOSITORY_ENV}");
    let repository = std::env::var(REPOSITORY_ENV).unwrap_or_default();
    let repository = repository.trim();
    if !repository.is_empty() {
        let path = repository.strip_prefix("https://github.com/")
            .expect("NOOKIT_GITHUB_URL must be an HTTPS GitHub repository URL");
        let parts: Vec<_> = path.trim_end_matches('/').split('/').collect();
        assert!(parts.len() == 2 && parts.iter().all(|part| {
            !part.is_empty() && *part != "." && *part != ".." &&
                part.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
        }), "NOOKIT_GITHUB_URL must point to a GitHub owner/repository");
    }
    println!("cargo:rustc-env={REPOSITORY_ENV}={repository}");
    tauri_build::build();
}
