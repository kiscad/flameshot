// SPDX-License-Identifier: GPL-3.0-or-later
//! Owned direct child processes. Pipe writes and waits never hold the registry lock.
use anyhow::{Context, Result, ensure};
use std::{
    io::Write,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
type Slot = Arc<Mutex<Option<Child>>>;
#[derive(Default)]
struct State {
    closing: bool,
    children: Vec<Slot>,
}
#[derive(Default)]
pub struct Children {
    state: Mutex<State>,
    live: AtomicUsize,
}
pub struct ShutdownGuard(Arc<Children>);
impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}
impl Children {
    pub fn guard(self: &Arc<Self>) -> ShutdownGuard {
        ShutdownGuard(self.clone())
    }
    pub fn spawn(self: &Arc<Self>, command: &mut Command, input: Option<Vec<u8>>) -> Result<u32> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Child registry lock poisoned"))?;
        ensure!(!state.closing, "Flameshot is shutting down");
        if input.is_some() {
            command.stdin(Stdio::piped());
        }
        let mut child = command.spawn().context("Cannot start Flameshot window")?;
        let pid = child.id();
        let pipe = child.stdin.take();
        let slot = Arc::new(Mutex::new(Some(child)));
        state
            .children
            .retain(|slot| slot.lock().unwrap_or_else(|e| e.into_inner()).is_some());
        state.children.push(slot.clone());
        self.live.fetch_add(1, Ordering::AcqRel);
        let registry = self.clone();
        std::thread::spawn(move || {
            if let (Some(mut pipe), Some(input)) = (pipe, input) {
                let _ = pipe.write_all(&input);
            }
            loop {
                {
                    let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
                    let done = match slot.as_mut() {
                        None => true,
                        Some(child) => match child.try_wait() {
                            Ok(Some(_)) => true,
                            Ok(None) => false,
                            Err(_) => {
                                let _ = child.kill();
                                let _ = child.wait();
                                true
                            }
                        },
                    };
                    if done {
                        slot.take();
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            registry.live.fetch_sub(1, Ordering::AcqRel);
        });
        Ok(pid)
    }
    pub fn idle(&self) -> bool {
        self.state
            .try_lock()
            .is_ok_and(|_state| self.live.load(Ordering::Acquire) == 0)
    }
    pub fn shutdown(&self) {
        let children = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.closing = true;
            std::mem::take(&mut state.children)
        };
        for slot in children {
            if let Some(mut child) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wait_idle(children: &Children) {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while !children.idle() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn shutdown_reaps_owned_children_even_with_a_blocked_input_pipe() {
        let children = Arc::new(Children::default());
        let guard = children.guard();
        let pid = children
            .spawn(
                Command::new("sh").args(["-c", "exec sleep 60"]),
                Some(vec![0; 2 * 1024 * 1024]),
            )
            .unwrap();
        assert!(!children.idle());
        std::thread::sleep(Duration::from_millis(50));
        drop(guard);
        wait_idle(&children);
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        assert!(children.spawn(&mut Command::new("true"), None).is_err());
        children.shutdown(); // Idempotent shutdown cannot target a reused PID.
    }
    #[test]
    fn racing_spawn_and_shutdown_cannot_leave_an_unregistered_child() {
        for _ in 0..10 {
            let children = Arc::new(Children::default());
            let _guard = children.guard();
            let barrier = Arc::new(std::sync::Barrier::new(2));
            let worker_children = children.clone();
            let worker_barrier = barrier.clone();
            let worker = std::thread::spawn(move || {
                worker_barrier.wait();
                worker_children.spawn(Command::new("sh").args(["-c", "exec sleep 60"]), None)
            });
            barrier.wait();
            children.shutdown();
            if let Ok(pid) = worker.join().unwrap() {
                assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
            }
            wait_idle(&children);
        }
    }
    #[test]
    fn natural_exit_and_failed_spawn_restore_idle_state() {
        let children = Arc::new(Children::default());
        let _guard = children.guard();
        children.spawn(&mut Command::new("true"), None).unwrap();
        wait_idle(&children);
        assert!(
            children
                .spawn(
                    &mut Command::new("/nonexistent/flameshot-test-program"),
                    None
                )
                .is_err()
        );
        assert!(children.idle());
    }
}
