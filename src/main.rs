use clap::Parser;
use rover::cli::Args;

#[tokio::main]
async fn main() -> rover::Result<()> {
    let args = Args::parse();

    println!("{args:#?}");

    args.run().await?;

    Ok(())
}
