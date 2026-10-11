use crate::{
    platform::{self, Streams},
    syntax::{self, Node, Part, Redirect, Result, Word},
};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_CAPTURE: u64 = 8 * 1024 * 1024;
const MAX_ARGUMENTS: usize = 4096;
#[derive(Clone)]
pub struct Context {
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub status: i32,
}
impl Context {
    pub fn initial(status: i32) -> Result<Self> {
        Ok(Self {
            cwd: std::env::current_dir().map_err(err)?,
            env: std::env::vars()
                .map(|(k, v)| (k.to_uppercase(), v))
                .collect(),
            status,
        })
    }
    pub fn get(&self, name: &str) -> String {
        let name = name.to_uppercase();
        if name == "PWD" {
            return self.cwd.to_string_lossy().into();
        }
        if name == "?" {
            return self.status.to_string();
        }
        self.env
            .get(&name)
            .or_else(|| {
                if name == "HOME" {
                    self.env.get("USERPROFILE")
                } else {
                    None
                }
            })
            .cloned()
            .unwrap_or_default()
    }
    fn set(&mut self, key: &str, value: String) {
        self.env.insert(key.to_uppercase(), value);
    }
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
#[derive(Default)]
struct TempStore {
    root: Option<PathBuf>,
    serial: u64,
}
impl TempStore {
    fn file(&mut self, bytes: &[u8]) -> Result<PathBuf> {
        if self.root.is_none() {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(err)?
                .as_nanos();
            let base = std::env::temp_dir();
            for attempt in 0..100 {
                let root = base.join(format!(
                    "clink-native-{}-{stamp}-{attempt}",
                    std::process::id()
                ));
                match fs::create_dir(&root) {
                    Ok(()) => {
                        self.root = Some(root);
                        break;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => return Err(err(e)),
                }
            }
        }
        let root = self
            .root
            .as_ref()
            .ok_or("Unable to create private temporary directory")?;
        self.serial += 1;
        let path = root.join(self.serial.to_string());
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(err)?;
        file.write_all(bytes).map_err(err)?;
        Ok(path)
    }
}
impl Drop for TempStore {
    fn drop(&mut self) {
        if let Some(root) = &self.root {
            let _ = fs::remove_dir_all(root);
        }
    }
}
pub struct Engine {
    tools: BTreeMap<String, Vec<String>>,
    temp: Mutex<TempStore>,
}
impl Engine {
    pub fn new(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .map_err(|e| format!("Tool manifest {}: {e}", path.display()))?;
        let mut tools = BTreeMap::new();
        for line in text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            let fields: Vec<_> = line.split('\t').map(str::to_owned).collect();
            if fields.len() < 2
                || !Path::new(&fields[1]).is_absolute()
                || fields.iter().any(|f| f.is_empty() || f.contains('\0'))
            {
                return Err("Tool manifest must contain name TAB absolute executable [TAB literal prefix argument]".into());
            }
            let extension = Path::new(&fields[1])
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !["exe", "com"].contains(&extension.as_str()) {
                return Err("Tool mappings must target native .exe/.com programs".into());
            }
            if tools
                .insert(fields[0].to_lowercase(), fields[1..].to_vec())
                .is_some()
            {
                return Err("Duplicate tool mapping".into());
            }
        }
        Ok(Self {
            tools,
            temp: Mutex::new(TempStore::default()),
        })
    }
    pub fn run(&self, node: &Node, ctx: &mut Context, streams: &Streams) -> Result<i32> {
        match node {
            Node::List(nodes) => {
                for node in nodes {
                    ctx.status = self.run(node, ctx, streams)?;
                }
            }
            Node::Conditional(nodes) => {
                for (op, node) in nodes {
                    if op.is_empty()
                        || (op == "&&" && ctx.status == 0)
                        || (op == "||" && ctx.status != 0)
                    {
                        ctx.status = self.run(node, ctx, streams)?;
                    }
                }
            }
            Node::Pipeline(nodes, links) if nodes.len() > 1 => {
                // Prepare all endpoints before starting any stage. Each worker owns
                // only its stdin/stdout/stderr; no writer leaks may keep EOF alive.
                let mut endpoints: Vec<_> = nodes
                    .iter()
                    .map(|_| streams.duplicate().map_err(err))
                    .collect::<Result<_>>()?;
                for i in 0..links.len() {
                    let (r, w) = platform::pipe().map_err(err)?;
                    endpoints[i].output = w;
                    endpoints[i + 1].input = r;
                }
                ctx.status = std::thread::scope(|scope| -> Result<i32> {
                    let mut jobs = Vec::new();
                    for (i, (node, io)) in nodes.iter().zip(endpoints).enumerate() {
                        let mut local = ctx.clone();
                        let merge = links.get(i).copied().unwrap_or(false);
                        jobs.push(scope.spawn(move || self.command(node, &mut local, io, merge)));
                    }
                    let mut last = 0;
                    for job in jobs {
                        last = job.join().map_err(|_| "Pipeline worker panicked")?;
                    }
                    Ok(last)
                })?;
            }
            Node::Pipeline(nodes, _) => {
                ctx.status = self.run(&nodes[0], ctx, streams)?;
            }
            Node::Command(_, _) => {
                ctx.status = self.command(node, ctx, streams.duplicate().map_err(err)?, false);
            }
        }
        Ok(ctx.status)
    }
    fn capture(&self, node: &Node, ctx: &Context, streams: &Streams) -> Result<Vec<u8>> {
        let (reader, writer) = platform::pipe().map_err(err)?;
        let mut io = streams.duplicate().map_err(err)?;
        io.output = writer;
        let mut local = ctx.clone();
        // Drain concurrently: a producer must not block on a full pipe while the
        // executor waits for its exit. Take at most limit+1 and close on overflow.
        let (result, bytes) = std::thread::scope(|scope| {
            let drain = scope.spawn(move || {
                let mut bytes = Vec::new();
                reader
                    .take(MAX_CAPTURE + 1)
                    .read_to_end(&mut bytes)
                    .map(|_| bytes)
            });
            let result = self.run(node, &mut local, &io);
            drop(io);
            (result, drain.join())
        });
        result?;
        let bytes = bytes.map_err(|_| "Capture worker panicked")?.map_err(err)?;
        if bytes.len() as u64 > MAX_CAPTURE {
            return Err("Substitution output exceeds 8 MiB".into());
        }
        Ok(bytes)
    }
    fn expand(
        &self,
        word: &Word,
        ctx: &Context,
        streams: &Streams,
        split: bool,
    ) -> Result<Vec<String>> {
        // Field splitting applies only to unquoted variable/capture values, not
        // literal text. Prefix/suffix concatenate with the first/last field.
        let mut fields: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut present = false;
        for part in &word.0 {
            let (value, quoted, dynamic) = match part {
                Part::Lit(s, q) => (s.clone(), *q, false),
                Part::Var(s, q) => (ctx.get(s), *q, true),
                Part::Capture(node, q) => {
                    let bytes = self.capture(node, ctx, streams)?;
                    let value = String::from_utf8(bytes)
                        .map_err(|_| "Command substitution requires UTF-8")?;
                    (value.trim_end_matches(['\r', '\n']).to_owned(), *q, true)
                }
                Part::Process(node) => {
                    let bytes = self.capture(node, ctx, streams)?;
                    let path = self.temp.lock().map_err(err)?.file(&bytes)?;
                    (path.to_string_lossy().into(), true, false)
                }
            };
            if value.contains('\0') {
                return Err("NUL cannot be passed in an argument".into());
            }
            if dynamic && !quoted && split {
                for c in value.chars() {
                    if matches!(c, ' ' | '\t' | '\n') {
                        if present {
                            fields.push(std::mem::take(&mut current));
                            present = false;
                        }
                    } else {
                        current.push(c);
                        present = true;
                    }
                }
            } else {
                present |= quoted || !value.is_empty();
                current.push_str(&value);
            }
        }
        if present {
            fields.push(current);
        }
        if fields.len() > MAX_ARGUMENTS {
            return Err("Too many expanded arguments".into());
        }
        Ok(fields)
    }
    fn command(&self, node: &Node, ctx: &mut Context, mut io: Streams, merge: bool) -> i32 {
        match self.command_inner(node, ctx, &mut io, merge) {
            Ok(status) => status,
            Err(message) => {
                let _ = writeln!(io.error, "clink-native: {message}");
                if message.starts_with("Command not found:") {
                    127
                } else {
                    2
                }
            }
        }
    }
    fn command_inner(
        &self,
        node: &Node,
        ctx: &mut Context,
        io: &mut Streams,
        merge: bool,
    ) -> Result<i32> {
        let Node::Command(words, redirects) = node else {
            return Err("Internal AST error".into());
        };
        let mut args = Vec::new();
        let mut assignments = Vec::new();
        let mut assigned = ctx.clone();
        let mut command_seen = false;
        for word in words {
            if !command_seen && word.assignment() {
                let value = self.expand(word, &assigned, io, false)?.join("");
                let (key, value) = value.split_once('=').ok_or("Invalid assignment")?;
                assigned.set(key, value.into());
                assignments.push((key.to_owned(), value.to_owned()));
            } else {
                command_seen = true;
                args.extend(self.expand(word, ctx, io, true)?);
                if args.len() > MAX_ARGUMENTS {
                    return Err("Too many command arguments".into());
                }
            }
        }
        for Redirect(op, word) in redirects {
            if op == "2>&1" {
                io.error = io.output.try_clone().map_err(err)?;
                continue;
            }
            if op == "1>&2" {
                io.output = io.error.try_clone().map_err(err)?;
                continue;
            }
            let target = self.expand(
                word.as_ref().ok_or("Missing redirect word")?,
                ctx,
                io,
                op != "<<<",
            )?;
            if target.len() != 1 {
                return Err("Ambiguous redirection target".into());
            }
            if op == "<<<" {
                let path = self
                    .temp
                    .lock()
                    .map_err(err)?
                    .file(format!("{}\n", target[0]).as_bytes())?;
                io.input = File::open(path).map_err(err)?;
                continue;
            }
            let path = if target[0] == "/dev/null" {
                PathBuf::from("NUL")
            } else {
                ctx.cwd.join(&target[0])
            };
            let mut options = OpenOptions::new();
            if op == "<" {
                options.read(true);
            } else {
                options.write(true).create(true);
                if op.contains(">>") {
                    options.append(true);
                } else {
                    options.truncate(true);
                }
            }
            let file = options
                .open(&path)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            if op.starts_with('&') {
                io.error = file.try_clone().map_err(err)?;
                io.output = file;
            } else if op == "<" {
                io.input = file;
            } else if op.starts_with('2') {
                io.error = file;
            } else {
                io.output = file;
            }
        }
        if merge {
            io.error = io.output.try_clone().map_err(err)?;
        }
        if args.is_empty() {
            for (key, value) in assignments {
                ctx.set(&key, value);
            }
            return Ok(0);
        }
        let active = if assignments.is_empty() {
            ctx
        } else {
            &mut assigned
        };
        syntax::validate_command(&args[0])?;
        match args[0].as_str() {
            "true" => Ok(0),
            "false" => Ok(1),
            "echo" => {
                let no_newline = args.get(1).is_some_and(|s| s == "-n");
                let text = args[if no_newline { 2 } else { 1 }..].join(" ")
                    + if no_newline { "" } else { "\n" };
                io.output.write_all(text.as_bytes()).map_err(err)?;
                Ok(0)
            }
            "pwd" => {
                writeln!(io.output, "{}", active.cwd.display()).map_err(err)?;
                Ok(0)
            }
            "cd" => {
                if args.len() > 2 {
                    return Err("cd expects at most one argument".into());
                }
                let value = args.get(1).cloned().unwrap_or_else(|| active.get("HOME"));
                let value = if value == "-" {
                    active.get("OLDPWD")
                } else {
                    value
                };
                if value.is_empty() {
                    return Err("cd target is empty".into());
                }
                let path = fs::canonicalize(active.cwd.join(value)).map_err(err)?;
                if !path.is_dir() {
                    return Err("cd target is not a directory".into());
                }
                let path = path.to_string_lossy();
                let normal = if let Some(p) = path.strip_prefix(r"\\?\UNC\") {
                    format!(r"\\{p}")
                } else {
                    path.strip_prefix(r"\\?\").unwrap_or(&path).to_owned()
                };
                active.set("OLDPWD", active.cwd.to_string_lossy().into());
                active.cwd = normal.into();
                Ok(0)
            }
            "export" | "unset" => {
                for arg in &args[1..] {
                    let (key, value) = arg.split_once('=').unwrap_or((arg, ""));
                    if !syntax::identifier(key) || (args[0] == "unset" && arg.contains('=')) {
                        return Err("Invalid environment variable name".into());
                    }
                    if args[0] == "unset" {
                        active.env.remove(&key.to_uppercase());
                    } else {
                        active.set(
                            key,
                            if arg.contains('=') {
                                value.into()
                            } else {
                                active.get(key)
                            },
                        );
                    }
                }
                Ok(0)
            }
            _ => {
                let invocation = self.resolve(&args[0], active)?;
                let status = Command::new(&invocation[0])
                    .args(&invocation[1..])
                    .args(&args[1..])
                    .current_dir(&active.cwd)
                    .env_clear()
                    .envs(&active.env)
                    .stdin(Stdio::from(io.input.try_clone().map_err(err)?))
                    .stdout(Stdio::from(io.output.try_clone().map_err(err)?))
                    .stderr(Stdio::from(io.error.try_clone().map_err(err)?))
                    .status()
                    .map_err(err)?;
                Ok(status.code().unwrap_or(130))
            }
        }
    }
    fn resolve(&self, name: &str, ctx: &Context) -> Result<Vec<String>> {
        let lower = name.to_lowercase();
        if lower.ends_with(".cmd") || lower.ends_with(".bat") {
            return Err("Batch files require explicit CMD mode".into());
        }
        if !name.contains(['/', '\\', ':'])
            && let Some(tool) = self.tools.get(&lower)
        {
            if !Path::new(&tool[0]).is_file() {
                return Err(format!("Configured executable is missing: {}", tool[0]));
            }
            return Ok(tool.clone());
        }
        let bases: Vec<PathBuf> = if name.contains(['/', '\\', ':']) {
            vec![ctx.cwd.join(name)]
        } else {
            std::env::split_paths(&ctx.get("PATH"))
                .filter(|p| !p.as_os_str().is_empty())
                .map(|p| ctx.cwd.join(p).join(name))
                .collect()
        };
        for base in bases {
            let candidates = if base.extension().is_some() {
                vec![base]
            } else {
                vec![base.with_extension("exe"), base.with_extension("com")]
            };
            for candidate in candidates {
                let ext = candidate
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ["exe", "com"].contains(&ext.as_str()) && candidate.is_file() {
                    return Ok(vec![candidate.to_string_lossy().into()]);
                }
            }
        }
        Err(format!("Command not found: {name}"))
    }
}
