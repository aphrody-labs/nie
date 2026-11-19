//! The child process of the terminal: a pty (`teletypewriter`: Unix pty or Windows ConPTY) driven
//! by a thread on rio's `corcovado` poller.
//!
//! The thread reads the child's output and forwards it as [`PtyEvent`]s, writes what the window
//! sends, and relays resizes. It knows nothing about windows or rendering: the caller passes a
//! `wake` callback (an event-loop proxy in the window, a no-op headless).

use std::collections::VecDeque;
use std::io::{self, ErrorKind, Read, Write};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread::JoinHandle;

use rio_vt::corcovado::channel::{self, Receiver, Sender};
use rio_vt::corcovado::{Events, Poll, PollOpt, Ready, Token};
use rio_vt::event::{Msg, WindowSize};
use rio_vt::teletypewriter::{EventedPty, ProcessReadWrite, WinsizeBuilder};

/// The program to run and where.
#[derive(Clone, Debug)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
}

/// Size of the pty in cells and pixels.
#[derive(Clone, Copy, Debug)]
pub struct PtySize {
    pub cols: u16,
    pub rows: u16,
    pub width: u16,
    pub height: u16,
}

/// What the child produced.
#[derive(Debug)]
pub enum PtyEvent {
    Output(Vec<u8>),
    /// The child exited, with the raw wait status when the platform gives one.
    Exited(Option<i32>),
}

/// A running child and the channels around it.
pub struct PtyHandle {
    tx: Sender<Msg>,
    pub events: mpsc::Receiver<PtyEvent>,
    thread: Option<JoinHandle<()>>,
}

const READ_BUFFER: usize = 64 * 1024;

fn environment() -> Vec<(String, String)> {
    vec![
        ("TERM".into(), "xterm-256color".into()),
        ("COLORTERM".into(), "truecolor".into()),
        ("TERM_PROGRAM".into(), "nie-term".into()),
    ]
}

#[cfg(unix)]
fn open(cmd: &CommandSpec, size: PtySize) -> io::Result<rio_vt::teletypewriter::Pty> {
    rio_vt::teletypewriter::create_pty_with_spawn(
        Some(&cmd.program),
        cmd.args.clone(),
        &cmd.cwd,
        Some(environment()),
        size.cols,
        size.rows,
        size.width,
        size.height,
    )
}

#[cfg(windows)]
fn open(cmd: &CommandSpec, size: PtySize) -> io::Result<rio_vt::teletypewriter::Pty> {
    rio_vt::teletypewriter::create_pty(Some(&cmd.program), cmd.args.clone(), &cmd.cwd, Some(environment()), size.cols, size.rows)
}

impl PtyHandle {
    /// Spawns the child on a pty of `size` and starts the I/O thread. `wake` runs after every event
    /// the thread queues.
    pub fn spawn(cmd: &CommandSpec, size: PtySize, wake: Arc<dyn Fn() + Send + Sync>) -> io::Result<Self> {
        let pty = open(cmd, size)?;
        let (tx, rx) = channel::channel();
        let (events_tx, events) = mpsc::channel();
        let thread = std::thread::Builder::new().name("nie-term-pty".into()).spawn(move || {
            let mut pty = pty;
            let result = run(&mut pty, &rx, &events_tx, &wake);
            if let Err(err) = result {
                eprintln!("nie term: pty thread: {err}");
                let _ = events_tx.send(PtyEvent::Exited(None));
                wake();
            }
            let _ = pty.shutdown();
        })?;
        Ok(Self {
            tx,
            events,
            thread: Some(thread),
        })
    }

    /// Queues bytes for the child's standard input.
    pub fn write(&self, bytes: Vec<u8>) {
        let _ = self.tx.send(Msg::Input(bytes.into()));
    }

    /// Resizes the pty; the child gets SIGWINCH (a console resize on Windows).
    pub fn resize(&self, size: PtySize) {
        let _ = self.tx.send(Msg::Resize(WindowSize {
            rows: size.rows,
            cols: size.cols,
            width: size.width,
            height: size.height,
        }));
    }

    /// Stops the thread and the child, and waits for them.
    pub fn close(&mut self) {
        let _ = self.tx.send(Msg::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for PtyHandle {
    fn drop(&mut self) {
        self.close();
    }
}

fn run<P: EventedPty>(
    pty: &mut P,
    rx: &Receiver<Msg>,
    out: &mpsc::Sender<PtyEvent>,
    wake: &Arc<dyn Fn() + Send + Sync>,
) -> io::Result<()> {
    let poll = Poll::new()?;
    let mut tokens = (0usize..).map(Token::from);
    let channel_token = tokens.next().unwrap_or(Token(0));
    poll.register(rx, channel_token, Ready::readable(), PollOpt::edge())?;
    pty.register(&poll, &mut tokens, Ready::readable(), PollOpt::level())?;

    let mut events = Events::with_capacity(256);
    let mut buf = vec![0u8; READ_BUFFER];
    let mut queue: VecDeque<Vec<u8>> = VecDeque::new();
    let mut written = 0usize;
    let mut last_interest = Ready::readable();

    loop {
        events.clear();
        if let Err(err) = poll.poll(&mut events, None) {
            if err.kind() == ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }

        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::Input(bytes) => queue.push_back(bytes.into_owned()),
                Msg::Resize(size) => {
                    let _ = pty.set_winsize(WinsizeBuilder::from(size));
                }
                Msg::Shutdown => return Ok(()),
            }
        }

        for event in events.iter() {
            let token = event.token();
            if token == channel_token {
                continue;
            }
            if token == pty.child_event_token() {
                if let Some(rio_vt::teletypewriter::ChildEvent::Exited(status)) = pty.next_child_event() {
                    drain(pty, &mut buf, out, wake);
                    let _ = out.send(PtyEvent::Exited(status));
                    wake();
                    return Ok(());
                }
                continue;
            }
            if token == pty.read_token() || token == pty.write_token() {
                #[cfg(unix)]
                let hung_up = rio_vt::corcovado::unix::UnixReady::from(event.readiness()).is_hup();
                #[cfg(not(unix))]
                let hung_up = false;
                if event.readiness().is_readable() || hung_up {
                    drain(pty, &mut buf, out, wake);
                }
                if !hung_up && event.readiness().is_writable() {
                    flush(pty, &mut queue, &mut written)?;
                }
            }
        }

        let mut interest = Ready::readable();
        if !queue.is_empty() {
            interest.insert(Ready::writable());
        }
        if interest != last_interest {
            pty.reregister(&poll, interest, PollOpt::level())?;
            last_interest = interest;
        }
    }
}

/// Reads everything the child has written so far.
fn drain<P: EventedPty>(pty: &mut P, buf: &mut [u8], out: &mpsc::Sender<PtyEvent>, wake: &Arc<dyn Fn() + Send + Sync>) {
    let mut sent = false;
    loop {
        match pty.reader().read(buf) {
            Ok(0) => break,
            Ok(n) => {
                let _ = out.send(PtyEvent::Output(buf[..n].to_vec()));
                sent = true;
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    if sent {
        wake();
    }
}

fn flush<P: EventedPty>(pty: &mut P, queue: &mut VecDeque<Vec<u8>>, written: &mut usize) -> io::Result<()> {
    while let Some(front) = queue.front() {
        match pty.writer().write(&front[*written..]) {
            Ok(0) => break,
            Ok(n) => {
                *written += n;
                if *written >= front.len() {
                    queue.pop_front();
                    *written = 0;
                }
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => {}
            Err(err) if err.kind() == ErrorKind::WouldBlock => break,
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn collect(handle: &PtyHandle, until_exit: bool) -> (Vec<u8>, Option<Option<i32>>) {
        let mut out = Vec::new();
        let mut exit = None;
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            match handle.events.recv_timeout(Duration::from_millis(200)) {
                Ok(PtyEvent::Output(bytes)) => out.extend(bytes),
                Ok(PtyEvent::Exited(status)) => {
                    exit = Some(status);
                    if until_exit {
                        break;
                    }
                }
                Err(_) if exit.is_some() => break,
                Err(_) => {}
            }
        }
        (out, exit)
    }

    #[test]
    fn printf_output_reaches_the_model() {
        let cmd = CommandSpec {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), r"printf '\033[31mhello\033[0m pty\n'".into()],
            cwd: None,
        };
        let size = PtySize {
            cols: 40,
            rows: 5,
            width: 320,
            height: 80,
        };
        let handle = PtyHandle::spawn(&cmd, size, Arc::new(|| {})).expect("spawn");
        let (bytes, exit) = collect(&handle, true);
        assert!(exit.is_some(), "the child exited");
        let mut model = nie_explore::term::TermModel::new(40, 5);
        model.feed(&bytes);
        assert_eq!(model.row_text(0), "hello pty");
        let frame = model.frame();
        assert_ne!(frame.cells[0][0].fg, frame.default_fg, "SGR red reached the cell");
    }

    #[test]
    fn input_is_echoed_by_cat() {
        let cmd = CommandSpec {
            program: "/bin/cat".into(),
            args: vec![],
            cwd: None,
        };
        let size = PtySize {
            cols: 40,
            rows: 5,
            width: 320,
            height: 80,
        };
        let handle = PtyHandle::spawn(&cmd, size, Arc::new(|| {})).expect("spawn");
        handle.write(b"ping\r".to_vec());
        let mut seen = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline && !String::from_utf8_lossy(&seen).contains("ping\r\nping") {
            if let Ok(PtyEvent::Output(bytes)) = handle.events.recv_timeout(Duration::from_millis(200)) {
                seen.extend(bytes);
            }
        }
        let text = String::from_utf8_lossy(&seen).into_owned();
        assert!(text.contains("ping\r\nping"), "echo plus cat output, got {text:?}");
        handle.resize(PtySize {
            cols: 80,
            rows: 24,
            width: 640,
            height: 384,
        });
    }
}
