//! Remote commands.

use crate::connect::Connection;
use crate::{Error, Result};
use russh::ChannelMsg;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// What a remote command printed and how it ended.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// Exit status; `None` when the server closed the channel without sending one (killed by a signal).
    pub status: Option<u32>,
}

impl Output {
    /// Standard output as text, lossily decoded.
    #[must_use]
    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    /// Standard error as text, lossily decoded.
    #[must_use]
    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

impl Connection {
    /// Runs `command` and collects its output.
    pub async fn exec(&self, command: &str) -> Result<Output> {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let status = self
            .exec_with(command, tokio::io::empty(), &mut out, &mut err)
            .await?;
        Ok(Output {
            stdout: out,
            stderr: err,
            status,
        })
    }

    /// Runs `command`, feeding `stdin` and streaming the output into `stdout` and `stderr` as it
    /// arrives. Returns the exit status.
    pub async fn exec_with<I, O, E>(
        &self,
        command: &str,
        stdin: I,
        stdout: &mut O,
        stderr: &mut E,
    ) -> Result<Option<u32>>
    where
        I: AsyncRead + Unpin + Send + 'static,
        O: AsyncWrite + Unpin,
        E: AsyncWrite + Unpin,
    {
        let mut channel = self
            .handle()
            .channel_open_session()
            .await
            .map_err(|e| Error::new(format!("ssh session: {e}")))?;
        channel
            .exec(true, command)
            .await
            .map_err(|e| Error::new(format!("ssh exec: {e}")))?;
        let writer = channel.make_writer();
        let feed = tokio::spawn(async move {
            let mut stdin = stdin;
            let mut writer = writer;
            let mut buf = vec![0u8; 32 * 1024];
            loop {
                match stdin.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if writer.write_all(&buf[..n]).await.is_err() {
                            return;
                        }
                    }
                }
            }
            let _ = writer.shutdown().await;
        });
        let mut status = None;
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::Data { data } => stdout.write_all(&data).await.map_err(io)?,
                ChannelMsg::ExtendedData { data, ext: 1 } => {
                    stderr.write_all(&data).await.map_err(io)?;
                }
                ChannelMsg::ExitStatus { exit_status } => status = Some(exit_status),
                _ => {}
            }
        }
        feed.abort();
        stdout.flush().await.map_err(io)?;
        stderr.flush().await.map_err(io)?;
        Ok(status)
    }
}

fn io(error: std::io::Error) -> Error {
    Error::new(format!("output: {error}"))
}

/// One blocking command for synchronous callers: resolves `host` (`[user@]host[:port]`) through
/// `~/.ssh/config`, connects, runs `command` with `stdin` and returns what it printed. It runs on a
/// private runtime and thread, so it is safe to call from inside another async runtime.
pub fn exec_blocking(host: &str, command: &str, stdin: Vec<u8>) -> Result<Output> {
    use crate::{HostKeyPolicy, Target, home_dir};
    let (host, command) = (host.to_owned(), command.to_owned());
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::new(format!("runtime: {e}")))?;
        runtime.block_on(async {
            let home = home_dir().ok_or_else(|| Error::new("HOME or USERPROFILE is not set"))?;
            let connection =
                Connection::connect(&Target::parse(&host)?, &home, HostKeyPolicy::Strict).await?;
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let status = connection
                .exec_with(&command, std::io::Cursor::new(stdin), &mut out, &mut err)
                .await?;
            Ok(Output {
                stdout: out,
                stderr: err,
                status,
            })
        })
    })
    .join()
    .map_err(|_| Error::new("ssh worker thread panicked"))?
}
