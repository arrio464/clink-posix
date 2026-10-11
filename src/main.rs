mod execute;
mod platform;
mod syntax;
use execute::{Context, Engine};
use std::{fs, io::Read, path::PathBuf};

fn hex(s: &str) -> String {
    s.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}
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
    let mut status = 0;
    let mut tools = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("tools.tsv");
    while let Some(option) = args.next() {
        if option == "--version" {
            println!("clink-posix-native 0.1.0; dialect 0.1; state protocol 1");
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
    let node = syntax::parse(&text)?;
    let engine = Engine::new(&tools)?;
    let mut ctx = Context::initial(status)?;
    let before = ctx.clone();
    let streams = platform::Streams::standard().map_err(|e| e.to_string())?;
    ctx.status = engine.run(&node, &mut ctx, &streams)?;
    // State is declarative data, never a generated CMD script. Publishing this
    // snapshot does not modify the parent process; an adapter must validate it.
    if let Some(path) = state {
        let mut data = format!(
            "V\t1\nR\t{}\nC\t{}\n",
            ctx.status,
            hex(&ctx.cwd.to_string_lossy())
        );
        for (key, value) in &ctx.env {
            if !key.starts_with('=') && before.env.get(key) != Some(value) {
                data += &format!("S\t{}\t{}\n", hex(key), hex(value));
            }
        }
        for key in before.env.keys() {
            if !key.starts_with('=') && !ctx.env.contains_key(key) {
                data += &format!("U\t{}\n", hex(key));
            }
        }
        // The caller supplies a unique, previously nonexistent output path.
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    }
    Ok(ctx.status)
}
