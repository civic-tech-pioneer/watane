//! Runs `install.sh` on a real pseudo-terminal with `script(1)` and types the answers (TECHNICAL 12.1).

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::{Fx, Out, RunPermit, WAIT_TIMEOUT};

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Starts the script under `script(1)`; for each step waits until its marker appears in the output,
/// then types the answer and Enter. Stdout and stderr arrive merged, with `\r` removed.
pub fn run(fx: &Fx, args: &[&str], env: &[(&str, &str)], steps: &[(&str, &str)]) -> Out {
    let steps: Vec<_> = steps
        .iter()
        .map(|(marker, answer)| (*marker, answer.as_bytes()))
        .collect();
    run_bytes(fx, args, env, &steps)
}

/// Types raw bytes, including answers that are not UTF-8.
pub fn run_bytes(fx: &Fx, args: &[&str], env: &[(&str, &str)], steps: &[(&str, &[u8])]) -> Out {
    let mut inner = format!("exec /bin/sh {}", quote(&fx.script.display().to_string()));
    for a in args {
        inner.push(' ');
        inner.push_str(&quote(a));
    }
    let mut c = Command::new("/usr/bin/script");
    if cfg!(target_os = "macos") {
        c.args(["-q", "/dev/null", "/bin/sh", "-c", &inner]);
    } else {
        c.args(["-qec", &inner, "/dev/null"]);
    }
    c.env_clear().envs(fx.env_pairs());
    for (k, v) in env {
        c.env(k, v);
    }
    c.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let _permit = RunPermit::acquire();
    let mut child = c.spawn().expect("spawn script(1)");
    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = child.stdout.take().expect("stdout");
    let buf: Arc<Mutex<Vec<u8>>> = Arc::default();
    let b2 = Arc::clone(&buf);
    let reader = thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        while let Ok(n) = stdout.read(&mut chunk) {
            if n == 0 {
                break;
            }
            b2.lock().expect("buf").extend_from_slice(&chunk[..n]);
        }
    });
    let text = |b: &Arc<Mutex<Vec<u8>>>| {
        String::from_utf8_lossy(&b.lock().expect("buf")).replace('\r', "")
    };
    let mut pos = 0;
    for (marker, answer) in steps {
        let t = Instant::now();
        loop {
            let s = text(&buf);
            if let Some(i) = s[pos..].find(marker) {
                pos += i + marker.len();
                break;
            }
            assert!(
                t.elapsed() < WAIT_TIMEOUT,
                "marker {marker:?} never came; output:\n{s}"
            );
            thread::sleep(Duration::from_millis(20));
        }
        // a script that asked nothing has already ended: typing then is harmless
        let _ = stdin.write_all(answer);
        let _ = stdin.write_all(b"\n");
        let _ = stdin.flush();
    }
    let status = child.wait().expect("wait");
    drop(stdin);
    reader.join().expect("reader");
    Out {
        code: status.code().unwrap_or(-1),
        stdout: text(&buf),
        stderr: String::new(),
    }
}
