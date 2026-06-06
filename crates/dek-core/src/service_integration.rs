#[cfg(target_os = "linux")]
pub fn notify_ready() {
    if let Err(e) = sd_notify::notify(true, &[sd_notify::NotifyState::Ready]) {
        tracing::warn!("Failed to notify systemd: {}", e);
    }
}

#[cfg(not(target_os = "linux"))]
pub fn notify_ready() {}

#[cfg(target_os = "windows")]
pub fn run_as_service_if_needed<F>(main_logic: F) -> anyhow::Result<()>
where
    F: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
    use windows_service::{
        define_windows_service,
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
    };
    use std::sync::mpsc;
    use std::ffi::OsString;
    use std::sync::Mutex;
    use std::thread;

    use std::sync::OnceLock;

    type ServiceLogic = Box<dyn FnOnce() + Send + 'static>;
    static SHUTDOWN_TX: OnceLock<Mutex<Option<mpsc::Sender<()>>>> = OnceLock::new();
    static LOGIC_WRAPPER: OnceLock<Mutex<Option<ServiceLogic>>> = OnceLock::new();

    define_windows_service!(ffi_service_main, my_service_main);

    fn my_service_main(arguments: Vec<OsString>) {
        if let Err(_e) = run_service(arguments) {
            // Log error
        }
    }

    fn run_service(_arguments: Vec<OsString>) -> anyhow::Result<()> {
        let (tx, rx) = mpsc::channel();
        let shutdown_tx_mutex = SHUTDOWN_TX.get_or_init(|| Mutex::new(None));
        *shutdown_tx_mutex.lock().unwrap() = Some(tx);

        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                windows_service::service::ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                windows_service::service::ServiceControl::Stop => {
                    if let Some(mutex) = SHUTDOWN_TX.get() {
                        if let Some(tx) = mutex.lock().unwrap().take() {
                            let _ = tx.send(());
                        }
                    }
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let status_handle = service_control_handler::register("PollenDEK", event_handler)?;
        
        let next_status = windows_service::service::ServiceStatus {
            service_type: windows_service::service::ServiceType::OWN_PROCESS,
            current_state: windows_service::service::ServiceState::Running,
            controls_accepted: windows_service::service::ServiceControlAccept::STOP,
            exit_code: windows_service::service::ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
            process_id: None,
        };
        status_handle.set_service_status(next_status.clone())?;

        // Extract the logic and spawn it
        if let Some(mutex) = LOGIC_WRAPPER.get() {
            if let Some(logic) = mutex.lock().unwrap().take() {
                thread::spawn(move || {
                    logic();
                    // When logic completes, we could send a signal to stop the service
                });
            }
        }

        // Block until SCM says stop
        let _ = rx.recv();

        let stop_status = windows_service::service::ServiceStatus {
            service_type: windows_service::service::ServiceType::OWN_PROCESS,
            current_state: windows_service::service::ServiceState::Stopped,
            controls_accepted: windows_service::service::ServiceControlAccept::empty(),
            exit_code: windows_service::service::ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
            process_id: None,
        };
        status_handle.set_service_status(stop_status)?;

        Ok(())
    }

    // We prepare the logic to be executed inside the thread.
    // However, the macro requires `ffi_service_main` and we can't easily pass state.
    // That's why we use the global LOGIC_WRAPPER.
    let rt = tokio::runtime::Runtime::new()?;
    
    let logic = Box::new(move || {
        match rt.block_on(main_logic) {
            Err(e) => {
                let _ = std::fs::write("C:\\ProgramData\\PollenDEK\\error.log", format!("Fatal error in core logic: {:?}", e));
            }
            Ok(()) => {
                let _ = std::fs::write("C:\\ProgramData\\PollenDEK\\error.log", "Core logic returned Ok(())");
            }
        }
    });

    let logic_mutex = LOGIC_WRAPPER.get_or_init(|| Mutex::new(None));
    *logic_mutex.lock().unwrap() = Some(logic);

    match service_dispatcher::start("PollenDEK", ffi_service_main) {
        Ok(_) => Ok(()),
        Err(_e) => {
            // Error 1063 means we are not running as a service.
            // If it's a different error, we should probably log it, but let's just run normally.
            
            // Re-extract the logic and run it in the current thread instead.
            if let Some(mutex) = LOGIC_WRAPPER.get() {
                if let Some(logic) = mutex.lock().unwrap().take() {
                    logic();
                }
            }
            Ok(())
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn run_as_service_if_needed<F>(main_logic: F) -> anyhow::Result<()>
where
    F: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(main_logic)
}
