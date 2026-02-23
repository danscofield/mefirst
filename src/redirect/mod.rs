mod ebpf;

use crate::config::Config;
use crate::error::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub enum RedirectMode {
    Ebpf(Arc<Mutex<ebpf::EbpfRedirector>>),
}

impl RedirectMode {
    pub fn from_config(config: &Config) -> Result<Self> {
        // eBPF is mandatory
        let redirector = ebpf::EbpfRedirector::new(config)?;
        Ok(RedirectMode::Ebpf(Arc::new(Mutex::new(redirector))))
    }

    pub async fn setup(&self) -> Result<()> {
        match self {
            RedirectMode::Ebpf(r) => {
                let mut redirector = r.lock().await;
                redirector.setup().await
            }
        }
    }

    pub async fn teardown(&self) -> Result<()> {
        match self {
            RedirectMode::Ebpf(r) => {
                let redirector = r.lock().await;
                redirector.teardown().await
            }
        }
    }
}
