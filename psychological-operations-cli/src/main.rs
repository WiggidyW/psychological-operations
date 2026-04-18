#[tokio::main]
async fn main() {
    match psychological_operations_cli::run().await {
        Ok(output) => println!("{output}"),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
