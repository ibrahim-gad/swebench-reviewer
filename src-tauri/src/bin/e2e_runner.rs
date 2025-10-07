use std::process;

// Include the e2e test module
#[path = "../../tests/e2e_tests.rs"]
mod e2e_tests;

#[tokio::main]
async fn main() {
    // Call the main function from e2e_tests
    if let Err(e) = e2e_tests::main().await {
        eprintln!("E2E tests failed: {}", e);
        process::exit(1);
    }
}
