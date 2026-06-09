use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

static SERVER_PROCESS: Mutex<Option<Child>> = Mutex::new(None);

const HOST: &str = "127.0.0.1";
const PORT: &str = "7860";
const SERVER_URL: &str = "http://127.0.0.1:7860";
// 90 seconds — models can take a while to initialise on first boot
const POLL_ATTEMPTS: u32 = 90;

/// Returns the Odysseus repo directory.
/// Checks ODYSSEUS_DIR env var first; falls back to $HOME/odysseus.
fn odysseus_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("ODYSSEUS_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
    PathBuf::from(home).join("odysseus")
}

/// Returns the path to the uvicorn binary inside the Odysseus venv.
fn uvicorn_bin() -> PathBuf {
    odysseus_dir().join("venv/bin/uvicorn")
}

pub fn run() {
    // Register a SIGTERM handler so graceful kills (system shutdown, launchctl stop, etc.)
    // also reap the uvicorn child before the process exits.
    unsafe {
        libc::signal(libc::SIGTERM, handle_sigterm as *const () as libc::sighandler_t);
    }

    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            // Run server startup in a background thread so the UI can render immediately.
            tauri::async_runtime::spawn_blocking(move || {
                start_and_wait(handle);
            });
            Ok(())
        })
        .on_window_event(|_window, event| {
            // Kill the server whenever the main window is destroyed (close button or Cmd+Q).
            if let tauri::WindowEvent::Destroyed = event {
                stop_server();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

extern "C" fn handle_sigterm(_: libc::c_int) {
    stop_server();
    std::process::exit(0);
}

fn start_and_wait(handle: AppHandle) {
    let dir = odysseus_dir();
    let uvicorn = uvicorn_bin();

    // Kill any stale process already holding the port before we try to start.
    let _ = Command::new("lsof")
        .args(["-ti", &format!("tcp:{}", PORT)])
        .output()
        .map(|o| {
            let pids = String::from_utf8_lossy(&o.stdout);
            for pid in pids.split_whitespace() {
                let _ = Command::new("kill").args(["-9", pid]).status();
            }
        });

    let result = Command::new(&uvicorn)
        .args(["app:app", "--host", HOST, "--port", PORT])
        .current_dir(&dir)
        .spawn();

    let child = match result {
        Err(e) => {
            let _ = handle.emit(
                "server-error",
                format!(
                    "Could not launch uvicorn at {}:\n{}",
                    uvicorn.display(),
                    e
                ),
            );
            return;
        }
        Ok(c) => c,
    };

    *SERVER_PROCESS.lock().unwrap() = Some(child);

    // Poll the TCP port — faster than HTTP and works before the ASGI stack is wired.
    let addr = format!("{}:{}", HOST, PORT);
    let mut ready = false;
    for _ in 0..POLL_ATTEMPTS {
        if TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(1)).is_ok() {
            ready = true;
            break;
        }
        // Check whether the child process already died (bad import, missing config, etc.)
        let exited = SERVER_PROCESS
            .lock()
            .map(|mut g| {
                g.as_mut()
                    .and_then(|c| c.try_wait().ok().flatten())
                    .is_some()
            })
            .unwrap_or(false);
        if exited {
            let _ = handle.emit(
                "server-error",
                "Odysseus exited unexpectedly during startup.\nCheck that the venv and requirements are installed.",
            );
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
    }

    if !ready {
        let _ = handle.emit(
            "server-error",
            format!("Server did not become ready after {} seconds.", POLL_ATTEMPTS),
        );
        return;
    }

    // Small grace period so the ASGI/HTTP stack fully initialises before we load it.
    std::thread::sleep(Duration::from_millis(600));

    if let Some(window) = handle.get_webview_window("main") {
        let _ = window.navigate(SERVER_URL.parse().unwrap());
    }
}

fn stop_server() {
    if let Ok(mut lock) = SERVER_PROCESS.lock() {
        if let Some(mut child) = lock.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
