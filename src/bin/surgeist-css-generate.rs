fn main() {
    if let Err(error) = surgeist_generator::css::run_from_env() {
        eprintln!("surgeist-css-generate: {error}");
        std::process::exit(i32::from(error.exit_code()));
    }
}
