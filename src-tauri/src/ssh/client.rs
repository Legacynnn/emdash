//! Thin wrapper around `openssh::Session`. The crate shells out to
//! the system `ssh` binary and uses a control socket for
//! multiplexing — no in-process SSH client.
//!
//! Implications:
//!
//! - Authentication is whatever the user's `ssh` binary supports
//!   (key, agent, password via SSH_ASKPASS, etc.).
//! - `private_key_path` is passed via `-i`; passphrase storage is
//!   our responsibility (`SshCredentials`) but openssh itself
//!   handles the prompt with no way to forward our stored value
//!   short of writing an `askpass` helper. v1 documents that
//!   passphrase-protected keys without agent forwarding aren't
//!   supported automatically; users should add the key to
//!   ssh-agent.
//! - On Windows the `openssh` crate refuses to compile in some
//!   configurations. We gate ssh operations behind a runtime
//!   `unsupported_on_windows` check and return
//!   `SshError::Unsupported` instead of panicking.

use std::time::{Duration, Instant};

use openssh::{KnownHosts, SessionBuilder};

use super::{ConnectionTestResult, SshConnection, SshError};

pub struct SshClient {
    session: openssh::Session,
}

impl SshClient {
    /// Open a session to the host. Honors the user's
    /// `~/.ssh/config` (openssh shells out to system `ssh`).
    pub async fn connect(connection: &SshConnection) -> Result<Self, SshError> {
        if cfg!(target_os = "windows") {
            return Err(SshError::Unsupported(
                "SSH support requires a POSIX `ssh` binary; Windows is not supported in v1",
            ));
        }
        let mut builder = SessionBuilder::default();
        builder
            .user(connection.username.clone())
            .port(connection.port)
            .known_hosts_check(KnownHosts::Add)
            .connect_timeout(Duration::from_secs(10));
        if let Some(key) = connection.private_key_path.as_deref() {
            builder.keyfile(key);
        }
        let session = builder.connect_mux(&connection.host).await?;
        Ok(Self { session })
    }

    /// Test-only: connect, measure latency, disconnect. No
    /// persisting on top of the user's `known_hosts` (`KnownHosts::Add`
    /// is still used so first-time hosts aren't rejected; the
    /// trade-off matches the Electron test path).
    pub async fn test_connection(connection: &SshConnection) -> ConnectionTestResult {
        let started = Instant::now();
        match Self::connect(connection).await {
            Ok(client) => {
                let latency = started.elapsed();
                // Best-effort disconnect.
                let _ = client.session.close().await;
                ConnectionTestResult {
                    success: true,
                    latency_ms: Some(latency.as_millis().min(u32::MAX as u128) as u32),
                    error: None,
                }
            }
            Err(e) => ConnectionTestResult {
                success: false,
                latency_ms: None,
                error: Some(e.to_string()),
            },
        }
    }

    /// Run a remote command and return stdout. Non-zero exit codes
    /// are surfaced as `SshError::Client`, matching the Electron
    /// behavior where command failures throw.
    pub async fn exec(&self, command: &str) -> Result<String, SshError> {
        let output = self
            .session
            .command("sh")
            .arg("-c")
            .arg(command)
            .output()
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(SshError::Client(format!(
                "remote exit status {}: {}",
                output.status,
                stderr.trim()
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Best-effort disconnect. Drops the underlying ssh control
    /// socket. Idempotent.
    pub async fn disconnect(self) -> Result<(), SshError> {
        self.session.close().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::AuthType;

    fn unreachable_connection() -> SshConnection {
        SshConnection {
            id: "test".to_string(),
            name: "test".to_string(),
            // RFC 5737 TEST-NET-1 — never routable.
            host: "192.0.2.1".to_string(),
            port: 22,
            username: "nobody".to_string(),
            auth_type: AuthType::Agent,
            private_key_path: None,
            use_agent: true,
            worktrees_dir: None,
            updated_at: "1970-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_connection_to_unreachable_host_reports_failure() {
        // We only assert the negative path because (a) a successful
        // path needs a real reachable host (CI-only flake) and (b)
        // the timeout is bounded to 10s by the SessionBuilder so this
        // test exits quickly without network.
        if cfg!(target_os = "windows") {
            let r = SshClient::test_connection(&unreachable_connection()).await;
            assert!(!r.success);
            assert!(r.error.unwrap().contains("not supported"));
            return;
        }
        let r = SshClient::test_connection(&unreachable_connection()).await;
        assert!(!r.success);
        assert!(r.latency_ms.is_none());
        assert!(r.error.is_some());
    }
}
