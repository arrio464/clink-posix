// A native child used to verify byte preservation, argv and real pipe behavior.
use std::io::{Read, Write};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args[0].as_str() {
        "args" => {
            for arg in &args[1..] {
                println!(
                    "{}",
                    arg.as_bytes()
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                );
            }
        }
        "emit" => {
            let n: usize = args[1].parse().unwrap();
            let data: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
            let mut out = std::io::stdout().lock();
            let mut left = n;
            while left > 0 {
                let count = left.min(data.len());
                if out.write_all(&data[..count]).is_err() {
                    break;
                }
                left -= count;
            }
        }
        "count" => {
            let mut n = 0;
            let mut buf = [0; 65536];
            let mut input = std::io::stdin().lock();
            loop {
                let k = input.read(&mut buf).unwrap();
                if k == 0 {
                    break;
                }
                n += k;
            }
            println!("{n}");
        }
        "cat" => {
            if args.len() == 1 {
                let _ = std::io::copy(&mut std::io::stdin().lock(), &mut std::io::stdout().lock());
            } else {
                for path in &args[1..] {
                    let mut file = std::fs::File::open(path).unwrap();
                    let _ = std::io::copy(&mut file, &mut std::io::stdout().lock());
                }
            }
        }
        "sort" => {
            let mut text = String::new();
            std::io::stdin().read_to_string(&mut text).unwrap();
            let mut lines: Vec<_> = text.lines().collect();
            lines.sort();
            for line in lines {
                println!("{line}");
            }
        }
        "printf" => {
            // Only the exact formats used in these tests; NOT a printf implementation.
            let format = &args[1];
            let output = if format == "%s\\n" {
                args[2..]
                    .iter()
                    .map(|arg| format!("{arg}\n"))
                    .collect::<String>()
            } else {
                assert!(
                    !format.contains('%'),
                    "Fixture does not support this printf format"
                );
                format.replace("\\n", "\n")
            };
            std::io::stdout().write_all(output.as_bytes()).unwrap();
        }
        "diff" => {
            assert_eq!(args.len(), 3);
            let equal = std::fs::read(&args[1]).unwrap() == std::fs::read(&args[2]).unwrap();
            std::process::exit(if equal { 0 } else { 1 });
        }
        "first" => {
            let mut b = [0; 1];
            std::io::stdin().read_exact(&mut b).unwrap();
            std::io::stdout().write_all(&b).unwrap();
        }
        "both" => {
            std::io::stdout().write_all(b"out\n").unwrap();
            std::io::stderr().write_all(b"err\n").unwrap();
        }
        "env" => println!("{}", std::env::var(&args[1]).unwrap_or_default()),
        "exit" => std::process::exit(args[1].parse().unwrap()),
        _ => panic!("unknown fixture command"),
    }
}
