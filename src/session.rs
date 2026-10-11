//! Parent transport: environment is hex data. The small CMD bridge changes only
//! cwd and exit status; it never receives the user's command or environment values.
use crate::{execute::Context, syntax::Result};
use std::{fs, io::Write, path::Path};
fn hex(s: &str) -> String {
    s.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}
fn publish(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        return Err("Session output already exists".into());
    }
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(".new");
    let temporary = std::path::PathBuf::from(temporary);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(bytes).map_err(|e| e.to_string())?;
        drop(file);
        // On Windows rename fails if the destination exists.
        fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
pub fn state(path: &Path, before: &Context, ctx: &Context) -> Result<()> {
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
    data += "E\n";
    publish(path, data.as_bytes())
}
pub fn sync(path: &Path, before: &Context, ctx: &Context) -> Result<()> {
    let mut data = String::from("@echo off\r\n");
    if ctx.cwd != before.cwd {
        let cwd = ctx.cwd.to_string_lossy();
        if cwd.contains(['\r', '\n', '"', '\0']) {
            return Err("Directory cannot be transported to CMD".into());
        }
        data += "@setlocal DisableDelayedExpansion\r\n";
        let unicode = !cwd.is_ascii();
        if unicode {
            data +=
                "@for /f \"tokens=2 delims=:\" %%C in ('chcp') do @set \"_CLINK_PX_CP=%%C\"\r\n";
            data += "@chcp 65001 >nul\r\n";
        }
        data += &format!("@endlocal & cd /d \"{}\"", cwd.replace('%', "%%"));
        if unicode {
            data += " & chcp %_CLINK_PX_CP% >nul";
        }
        data += "\r\n";
    }
    data += &format!("@exit /b {}\r\n", ctx.status);
    publish(path, data.as_bytes())
}
