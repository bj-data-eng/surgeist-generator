fn main() {
    if let Err(error) = surgeist_generator::layout::run_from_env() {
        eprintln!("surgeist-layout-generate: {error}");
        std::process::exit(i32::from(error.exit_code()));
    }
}
