use std::io::BufRead;

use crate::libs::config::Storage;
use crate::libs::db::{self, Database};
use crate::libs::password;

const USAGE: &str = "\
Usage:
  oauth-rs                       start the server
  oauth-rs create-user --email <email> --username <username> [--password-stdin]
  oauth-rs help

create-user writes directly to the configured database (STORAGE_BACKEND,
DATABASE_URL / SQLITE_PATH from the environment or .env). No running server
or invite code is needed. The password is prompted for twice, or read from
the first line of stdin with --password-stdin.";

/// Runs a CLI command and returns the process exit code.
pub async fn run(args: &[String]) -> i32 {
    match args[0].as_str() {
        "create-user" => match parse_create_user(&args[1..]) {
            Ok(opts) => create_user(opts).await,
            Err(e) => {
                eprintln!("error: {}\n\n{}", e, USAGE);
                2
            }
        },
        "help" | "-h" | "--help" => {
            println!("{}", USAGE);
            0
        }
        other => {
            eprintln!("error: unknown command '{}'\n\n{}", other, USAGE);
            2
        }
    }
}

#[derive(Debug, PartialEq)]
struct CreateUser {
    email:          String,
    username:       String,
    password_stdin: bool,
}

fn parse_create_user(args: &[String]) -> Result<CreateUser, String> {
    let mut email = None;
    let mut username = None;
    let mut password_stdin = false;

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--email"          => email = Some(it.next().ok_or("--email needs a value")?.clone()),
            "--username"       => username = Some(it.next().ok_or("--username needs a value")?.clone()),
            "--password-stdin" => password_stdin = true,
            other => return Err(format!("unknown option '{}'", other)),
        }
    }

    let email = email.map(|e| e.trim().to_string()).filter(|e| !e.is_empty()).ok_or("--email is required")?;
    let username = username.map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).ok_or("--username is required")?;

    Ok(CreateUser { email, username, password_stdin })
}

fn read_password(from_stdin: bool) -> Result<String, String> {
    let password = if from_stdin {
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line).map_err(|e| e.to_string())?;
        line.trim_end_matches(['\r', '\n']).to_string()
    } else {
        let first = rpassword::prompt_password("Password: ").map_err(|e| e.to_string())?;
        let second = rpassword::prompt_password("Confirm password: ").map_err(|e| e.to_string())?;
        if first != second {
            return Err("passwords do not match".to_string());
        }
        first
    };

    if password.is_empty() {
        return Err("password must not be empty".to_string());
    }
    Ok(password)
}

async fn create_user(opts: CreateUser) -> i32 {
    let password = match read_password(opts.password_stdin) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {}", e);
            return 1;
        }
    };

    let database = match Storage::load() {
        Storage::Postgres { database_url, .. } => Database::Postgres(db::connect(&database_url).await),
        Storage::Sqlite { path }               => Database::Sqlite(db::connect_sqlite(&path).await),
    };
    database.run_migrations().await;

    match database.create_user(&opts.email, &opts.username, &password::hash(&password)).await {
        Ok(id) => {
            println!("Created user {} ({})", opts.username, id);
            0
        }
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            eprintln!("error: email or username already taken");
            1
        }
        Err(e) => {
            eprintln!("error: failed to create user: {}", e);
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_parse_create_user() {
        assert_eq!(
            parse_create_user(&args(&["--email", "a@x.dev", "--username", "alice"])),
            Ok(CreateUser { email: "a@x.dev".into(), username: "alice".into(), password_stdin: false }),
        );
        assert_eq!(
            parse_create_user(&args(&["--password-stdin", "--username", "alice", "--email", "a@x.dev"])),
            Ok(CreateUser { email: "a@x.dev".into(), username: "alice".into(), password_stdin: true }),
        );
        assert!(parse_create_user(&args(&["--email", "a@x.dev"])).is_err());
        assert!(parse_create_user(&args(&["--email", " ", "--username", "alice"])).is_err());
        assert!(parse_create_user(&args(&["--email"])).is_err());
        assert!(parse_create_user(&args(&["--email", "a@x.dev", "--username", "alice", "--bogus"])).is_err());
    }
}
