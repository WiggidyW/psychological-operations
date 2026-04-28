#[tokio::main]
async fn main() {
    match psychological_operations_cli::run(std::env::args_os()).await {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
