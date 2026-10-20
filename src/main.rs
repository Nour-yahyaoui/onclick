//! `store.exe` — entry point and operator CLI.
//!
//! Usage:
//!   store                 start the till (default port 8642)
//!   store --port 9000     another port
//!   store --data D:\shop  data folder (default: ./data)
//!   store --doctor        self-check, prints what is wrong
//!   store --version       which build is this
//!   store --help

use anyhow::Result;
use std::io::Write;
use std::path::PathBuf;

/// Windows only: the console window must die with the app. There is no GUI
/// window to own, so without this the black window stays behind after a crash
/// or a Ctrl-C, and the cashier thinks the till is still running.
#[cfg(windows)]
fn keep_console_tidy() {
    unsafe extern "system" {
        fn SetConsoleCtrlHandler(handler: Option<unsafe extern "system" fn(u32) -> i32>, add: i32) -> i32;
    }
    unsafe extern "system" fn closing(_kind: u32) -> i32 {
        1
    }
    unsafe {
        SetConsoleCtrlHandler(Some(closing), 1);
    }
}

#[cfg(not(windows))]
fn keep_console_tidy() {}

fn main() -> Result<()> {
    let mut data_dir = PathBuf::from("data");
    let mut port: u16 = std::env::var("STORE_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8642);
    let mut run_doctor = false;
    let mut open_browser = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--doctor" => run_doctor = true,
            "--port" => {
                if let Some(value) = args.next() {
                    port = value.parse().unwrap_or(port);
                }
            }
            "--data" => {
                if let Some(value) = args.next() {
                    data_dir = PathBuf::from(value);
                }
            }
            // Double-clicking store.exe must open the shop, not ask a question.
            "--open" => open_browser = true,
            "--version" | "-V" => {
                println!("OneClick {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other => {
                eprintln!("Option inconnue : {other}\n");
                print_help();
                std::process::exit(2);
            }
        }
    }

    if run_doctor {
        return doctor(&data_dir);
    }

    keep_console_tidy();

    // Chrome / Edge block Cloudflare-style tunnels, so the browser is opened by
    // the OS, not from JavaScript.
    if open_browser {
        let url = format!("http://localhost:{port}");
        let _ = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(900));
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("cmd")
                    .args(["/C", "start", "", &url])
                    .spawn();
            }
            #[cfg(not(windows))]
            {
                let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
            }
        });
    }

    println!("Fermez cette fenêtre pour arrêter la caisse.");
    let _ = std::io::stdout().flush();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(app::serve(data_dir, port))
}

fn print_help() {
    println!(
        "OneClick {version} — caisse et gestion de magasin (local, hors ligne)\n\
         \n\
         store --version       afficher la version\n\
         store                 démarrer la caisse\n\
         store --open          démarrer et ouvrir le navigateur\n\
         store --port 9000     changer le port (défaut 8642)\n\
         store --data D:\\shop  dossier des données (défaut ./data)\n\
         store --doctor        diagnostic de la base et de l'installation\n",
        version = env!("CARGO_PKG_VERSION")
    );
}

fn doctor(data_dir: &PathBuf) -> Result<()> {
    let db_path = data_dir.join("store.db");
    println!("OneClick — diagnostic");
    println!("dossier : {}", data_dir.display());
    println!("base    : {}", db_path.display());

    let conn = storage::open(&db_path)?;
    storage::migrate(&conn)?;
    println!("schéma  : v{}", storage::schema_version(&conn)?);

    let problems = storage::doctor(&conn, &db_path)?;
    if problems.is_empty() {
        println!("état    : OK — rien à signaler");
    } else {
        println!("état    : {} point(s) à vérifier", problems.len());
        for problem in problems {
            println!("  ⚠ {problem}");
        }
    }
    Ok(())
}
