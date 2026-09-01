//! `org.loveyu.DevTools` D-Bus 服务：对外的 Worker 控制接口（打开工具、设置、重启等）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use devtools_core::{WORKER_INTERFACE, WORKER_OBJECT_PATH, WORKER_SERVICE_NAME};
use tao::event_loop::EventLoopProxy;
use zbus::blocking::{Connection, ConnectionBuilder, Proxy};

use crate::ipc::tool_event;
use crate::registry::ToolRegistry;
use crate::UserEvent;

struct WorkerService {
    proxy: EventLoopProxy<UserEvent>,
    registry: Arc<ToolRegistry>,
    webview_ready: Arc<AtomicBool>,
}

#[zbus::interface(name = "org.loveyu.DevTools")]
impl WorkerService {
    fn OpenLauncher(&self) -> zbus::fdo::Result<()> {
        send_event(&self.proxy, UserEvent::OpenLauncher)
    }

    fn OpenQuickInput(&self) -> zbus::fdo::Result<()> {
        send_event(&self.proxy, UserEvent::OpenQuickInput)
    }

    fn OpenTool(&self, tool: &str, payload: &str) -> zbus::fdo::Result<()> {
        let event = tool_event(&self.registry, tool, payload).map_err(zbus::fdo::Error::Failed)?;
        send_event(&self.proxy, event)
    }

    fn OpenSettings(&self) -> zbus::fdo::Result<()> {
        send_event(&self.proxy, UserEvent::OpenSettings)
    }

    fn Restart(&self) -> zbus::fdo::Result<()> {
        send_event(&self.proxy, UserEvent::Restart)
    }

    fn Quit(&self) -> zbus::fdo::Result<()> {
        send_event(&self.proxy, UserEvent::Quit)
    }

    fn IsWebViewReady(&self) -> bool {
        self.webview_ready.load(Ordering::Acquire)
    }
}

fn send_event(proxy: &EventLoopProxy<UserEvent>, event: UserEvent) -> zbus::fdo::Result<()> {
    proxy
        .send_event(event)
        .map_err(|_| zbus::fdo::Error::Failed("worker event loop is closed".to_owned()))
}

pub struct IpcGuard {
    _connection: Connection,
}

pub fn start_ipc(
    proxy: EventLoopProxy<UserEvent>,
    registry: Arc<ToolRegistry>,
    webview_ready: Arc<AtomicBool>,
) -> Result<IpcGuard, Box<dyn std::error::Error>> {
    let service = WorkerService {
        proxy,
        registry,
        webview_ready,
    };
    let connection = ConnectionBuilder::session()?
        .name(WORKER_SERVICE_NAME)?
        .serve_at(WORKER_OBJECT_PATH, service)?
        .build()?;
    Ok(IpcGuard {
        _connection: connection,
    })
}

pub fn request_existing_worker(method: &str) -> bool {
    let request = || -> zbus::Result<()> {
        let connection = Connection::session()?;
        let proxy = Proxy::new(
            &connection,
            WORKER_SERVICE_NAME,
            WORKER_OBJECT_PATH,
            WORKER_INTERFACE,
        )?;
        proxy.call::<_, _, ()>(method, &())
    };
    request().is_ok()
}
