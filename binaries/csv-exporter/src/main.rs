use anyhow::Result;
use clap::{Parser, Subcommand};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use std::{env, path::PathBuf};
use tokio_postgres::NoTls;
use tracing::info;

#[derive(Parser)]
#[command(name = "csv-exporter")]
#[command(about = "Export data from Collider database to CSV files for load testing")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output directory for CSV files
    #[arg(short, long, default_value = "./load_testing/data")]
    output_dir: PathBuf,

    /// Limit number of users to export (default: all)
    #[arg(long)]
    user_limit: Option<u64>,

    /// Limit number of event types to export (default: all)
    #[arg(long)]
    event_type_limit: Option<u64>,
}

#[derive(Subcommand)]
enum Commands {
    /// Export users to CSV
    Users,
    /// Export event types to CSV
    EventTypes,
    /// Export both users and event types to CSV
    All,
}

async fn create_pool() -> Result<Pool> {
    dotenvy::dotenv().ok();
    let database_url = env::var("DATABASE_URL")?;
    let pg_cfg = database_url.parse::<tokio_postgres::Config>()?;

    let mgr_config = ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    };

    let mgr = Manager::from_config(pg_cfg, NoTls, mgr_config);
    let pool = Pool::builder(mgr)
        .max_size(16)
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .build()?;

    Ok(pool)
}

async fn export_users(pool: &Pool, output_dir: &PathBuf, limit: Option<u64>) -> Result<()> {
    info!("Exporting users...");
    
    let client = pool.get().await?;
    let limit_clause = limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default();
    let sql = format!("SELECT id, name, created_at FROM users ORDER BY created_at{}", limit_clause);
    
    let rows = client.query(&sql, &[]).await?;
    let count = rows.len();
    
    std::fs::create_dir_all(output_dir)?;
    let csv_path = output_dir.join("users.csv");
    let mut csv_content = String::from("id,name,email,created_at\n");
    
    for row in rows {
        let id: uuid::Uuid = row.get(0);
        let name: String = row.get(1);
        let created_at: chrono::DateTime<chrono::Utc> = row.get(2);
        let email = format!("{}@example.com", name.replace(' ', "").to_lowercase());
        
        csv_content.push_str(&format!(
            "\"{}\",\"{}\",\"{}\",\"{}\"\n",
            id, name, email, created_at.to_rfc3339()
        ));
    }
    
    std::fs::write(&csv_path, csv_content)?;
    info!("Exported {} users to: {}", count, csv_path.display());
    Ok(())
}

async fn export_event_types(pool: &Pool, output_dir: &PathBuf, limit: Option<u64>) -> Result<()> {
    info!("Exporting event types...");
    
    let client = pool.get().await?;
    let limit_clause = limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default();
    let sql = format!("SELECT id, name FROM event_types ORDER BY id{}", limit_clause);
    
    let rows = client.query(&sql, &[]).await?;
    let count = rows.len();
    
    std::fs::create_dir_all(output_dir)?;
    let csv_path = output_dir.join("event_types.csv");
    let mut csv_content = String::from("id,name,description,count\n");
    
    for row in rows {
        let id: i32 = row.get(0);
        let name: String = row.get(1);
        let count = 99990 + (id % 10) as u64;
        
        csv_content.push_str(&format!(
            "{},\"{}\",\"Auto-generated from live data\",{}\n",
            id, name, count
        ));
    }
    
    std::fs::write(&csv_path, csv_content)?;
    info!("Exported {} event types to: {}", count, csv_path.display());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let cli = Cli::parse();
    let pool = create_pool().await?;
    
    match cli.command {
        Commands::Users => export_users(&pool, &cli.output_dir, cli.user_limit).await?,
        Commands::EventTypes => export_event_types(&pool, &cli.output_dir, cli.event_type_limit).await?,
        Commands::All => {
            export_users(&pool, &cli.output_dir, cli.user_limit).await?;
            export_event_types(&pool, &cli.output_dir, cli.event_type_limit).await?;
        }
    }
    
    info!("Done!");
    Ok(())
}