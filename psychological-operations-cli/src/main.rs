#[tokio::main]
async fn main() {
    match psychological_operations_cli::run().await {
        Ok(output) => {
            let s = output.to_string();
            if !s.is_empty() {
                println!("{s}");
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
