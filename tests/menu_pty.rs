#![cfg(all(feature = "tui-doctor", feature = "tui-menu"))]

use assert_cmd::Command;
use parking_lot::Mutex;
use portable_pty::{Child, CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;
use std::time::{Duration, Instant};

struct PtyApp {
    child: Box<dyn Child + Send>,
    writer: Box<dyn Write + Send>,
    screen: Arc<Mutex<vt100::Parser>>,
}

fn fixture_home() -> TempDir {
    let home = TempDir::new().unwrap();
    let global = home.path().join(".agents").join("skills");
    for name in ["demo-review", "demo-scan"] {
        let src = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("testdata/skills")
                .join(name)
                .join("SKILL.md"),
        )
        .unwrap();
        std::fs::create_dir_all(global.join(name)).unwrap();
        std::fs::write(global.join(name).join("SKILL.md"), src).unwrap();
    }
    home
}

fn spawn_menu(home: &Path, cwd: &Path) -> PtyApp {
    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");
    let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_lunchbox"));
    cmd.arg("menu");
    cmd.env("HOME", home);
    cmd.env("TERM", "xterm-256color");
    cmd.cwd(cwd);
    let child = pair.slave.spawn_command(cmd).expect("spawn menu");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("clone reader");
    let screen: Arc<Mutex<vt100::Parser>> =
        Arc::new(Mutex::new(vt100::Parser::new(24, 80, 0)));
    let sink = Arc::clone(&screen);
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => sink.lock().process(&buf[..n]),
            }
        }
    });
    let writer = pair.master.take_writer().expect("take writer");
    PtyApp {
        child,
        writer,
        screen,
    }
}

fn send(app: &mut PtyApp, keys: &str) {
    app.writer.write_all(keys.as_bytes()).unwrap();
    app.writer.flush().unwrap();
}

fn expect(app: &PtyApp, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        {
            let parser = app.screen.lock();
            let contents = parser.screen().contents();
            if contents.contains(needle) {
                return;
            }
        }
        if Instant::now() > deadline {
            let contents = app.screen.lock().screen().contents();
            panic!("expected {needle:?} within 10s; screen:\n{contents}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn wait_exit(app: &mut PtyApp) -> bool {
    app.child.wait().unwrap().success()
}

fn scratch() -> TempDir {
    TempDir::new().unwrap()
}

#[test]
fn menu_e2e_start_flow() {
    let home = fixture_home();
    let cwd = scratch();
    let mut app = spawn_menu(home.path(), cwd.path());
    expect(&app, "Pantry");
    send(&mut app, "\r");
    expect(&app, "demo-review");
    send(&mut app, " ");
    send(&mut app, "\x1b[B");
    send(&mut app, " ");
    send(&mut app, "s");
    expect(&app, "sha256:");
    expect(&app, "menu_tokens    this run: 27");
    send(&mut app, "\r");
    expect(&app, "mounted");
    expect(&app, "lbx_");
    send(&mut app, "f");
    expect(&app, "unmounted — result.json written");
    send(&mut app, "q");
    assert!(wait_exit(&mut app));
    drop(app);

    let runs = home.path().join(".lunchbox").join("runs");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&runs)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();
    assert_eq!(dirs.len(), 1, "exactly one run dir: {dirs:?}");
    let run_dir = dirs.pop().unwrap();
    assert!(!run_dir.join("workdir").exists(), "workdir unmounted");
    let result: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("result.json")).unwrap())
            .unwrap();
    assert_eq!(result["unmounted"], serde_json::json!(true));
}

#[test]
fn menu_e2e_requires_tty() {
    let home = scratch();
    let cwd = scratch();
    Command::cargo_bin("lunchbox")
        .unwrap()
        .arg("menu")
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("menu requires a terminal"));
}

#[test]
fn menu_e2e_doctor_route() {
    let home = fixture_home();
    let cwd = scratch();
    let mut app = spawn_menu(home.path(), cwd.path());
    expect(&app, "Pantry");
    send(&mut app, "\r");
    expect(&app, "demo-review");
    send(&mut app, "\x1b");
    expect(&app, "Pantry");
    send(&mut app, "\x1b[B\x1b[B");
    send(&mut app, "\r");
    expect(&app, "lunchbox doctor");
    send(&mut app, "q");
    expect(&app, "lunchbox menu");
    send(&mut app, "q");
    assert!(wait_exit(&mut app));
}

