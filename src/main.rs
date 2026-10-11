mod aliases;
mod arithmetic;
mod execute;
mod expansion;
mod platform;
mod session;
mod syntax;
use execute::{Context, Engine};
use std::{fs, io::Read, path::PathBuf};

fn main() {
    match entry() {
        Ok(code) => std::process::exit(code),
        Err(message) => {
            eprintln!("clink-native: {message}");
            std::process::exit(2);
        }
    }
}
fn entry() -> Result<i32, String> {
    let mut args = std::env::args().skip(1);
    let mut command = None;
    let mut request = None;
    let mut state = None;
    let mut sync = None;
    let mut alias_path = None;
    let mut status = 0;
    let mut tools = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("tools.tsv");
    while let Some(option) = args.next() {
        if option == "--version" {
            println!("clink-posix-native 0.2.0; dialect 0.2; state protocol 1");
            return Ok(0);
        }
        let value = args
            .next()
            .ok_or_else(|| format!("Missing value for {option}"))?;
        match option.as_str() {
            "-c" => command = Some(value),
            "--request" => request = Some(value),
            "--tools" => tools = PathBuf::from(value),
            "--state" => state = Some(PathBuf::from(value)),
            "--sync" => sync = Some(PathBuf::from(value)),
            "--aliases" => alias_path = Some(PathBuf::from(value)),
            "--status" => status = value.parse().map_err(|_| "Invalid status")?,
            _ => return Err(format!("Unknown option {option}")),
        }
    }
    if command.is_some() == request.is_some() {
        return Err("Specify exactly one of -c TEXT or --request UTF8_FILE".into());
    }
    let text = match command {
        Some(s) => s,
        None => {
            let mut bytes = Vec::new();
            fs::File::open(request.unwrap())
                .map_err(|e| e.to_string())?
                .take(syntax::MAX_INPUT as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            String::from_utf8(bytes).map_err(|_| "Request must be UTF-8")?
        }
    };
    let mut ctx = Context::initial(status)?;
    let before = ctx.clone();
    let result = (|| {
        let mut node = syntax::parse(&text)?;
        if let Some(path) = alias_path {
            aliases::expand(&mut node, &aliases::read(&path)?)?;
        }
        let engine = Engine::new(&tools)?;
        let streams = platform::Streams::standard().map_err(|e| e.to_string())?;
        engine.run(&node, &mut ctx, &streams)
    })();
    ctx.status = result.as_ref().copied().unwrap_or(2);
    if result.is_ok()
        && let Some(path) = state
    {
        session::state(&path, &before, &ctx)?;
    }
    if let Some(path) = sync {
        session::sync(&path, &before, &ctx)?;
    }
    result
}
