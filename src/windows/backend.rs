//! Native Windows backend implementing priority/scheduling reads & writes and window closure.
use super::process::{alive, enumerate, exact_handle, last_fault, Handle, Safety};
use crate::{engine::Backend, model::*, policy};
use std::{
    collections::HashSet,
    mem::size_of,
    ptr::null_mut,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::Threading::*,
    UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, WM_CLOSE},
};

#[link(name = "gdi32")]
unsafe extern "system" {
    fn D3DKMTGetProcessSchedulingPriorityClass(process: HANDLE, priority: *mut i32) -> i32;
    fn D3DKMTSetProcessSchedulingPriorityClass(process: HANDLE, priority: i32) -> i32;
}

pub struct WindowsBackend {
    safety: Safety,
    protected_paths: Vec<String>,
    guarded: HashSet<u32>,
    game: Option<Identity>,
    manual_mode: bool,
    cancel: Option<Arc<AtomicBool>>,
}

impl WindowsBackend {
    pub fn new(
        game: Option<Identity>,
        protected_paths: Vec<String>,
        cancel: Option<Arc<AtomicBool>>,
    ) -> NativeResult<Self> {
        let rows = enumerate()?;
        let guarded = game
            .as_ref()
            .map(|g| policy::guarded_pids(&rows, g))
            .unwrap_or_default();
        Ok(Self {
            safety: Safety::new()?,
            protected_paths,
            guarded,
            game,
            manual_mode: false,
            cancel,
        })
    }
    pub fn set_manual_mode(&mut self, enabled: bool) {
        self.manual_mode = enabled;
    }
    fn checked(&self, id: &Identity, rights: u32) -> NativeResult<Handle> {
        let h = exact_handle(id, rights)?;
        self.safety.check(h.0, id)?;
        if self.guarded.contains(&id.pid)
            || self
                .protected_paths
                .iter()
                .any(|p| policy::normalized_path(p) == policy::normalized_path(&id.path))
        {
            return Err(Fault::new(
                FaultKind::Denied,
                "Game family or user-protected executable.",
            ));
        }
        Ok(h)
    }
    fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|v| v.load(Ordering::Relaxed))
            || self
                .game
                .as_ref()
                .is_some_and(|g| !matches!(alive(g), Ok(true)))
    }
}

impl Backend for WindowsBackend {
    fn validate_reopen(&mut self, id: &Identity, approval: &ReopenApproval) -> NativeResult<()> {
        self.authorize(id)?;
        super::reopen::validate_approval(id, approval)
    }
    fn reopen(&mut self, id: &Identity, approval: &ReopenApproval) -> NativeResult<ReopenRecord> {
        super::reopen::reopen(id, approval)
    }
    fn authorize(&mut self, id: &Identity) -> NativeResult<()> {
        if self.cancelled() {
            return Err(Fault::new(
                FaultKind::Denied,
                "Session cancelled or game no longer running.",
            ));
        }
        if self.game.is_some() || self.manual_mode {
            let settings = super::runner::database()
                .and_then(|db| db.settings())
                .map_err(|e| {
                    Fault::new(
                        FaultKind::Denied,
                        format!("Cannot revalidate user protection: {e}"),
                    )
                })?;
            for path in settings.protected_paths {
                if !self.protected_paths.contains(&path) {
                    self.protected_paths.push(path);
                }
            }
        }
        if self.manual_mode {
            let _ = super::manual::authorize_current_rule(id)?;
        }
        self.checked(id, 0).map(|_| ())
    }
    fn read(&mut self, id: &Identity, property: Property) -> NativeResult<Value> {
        let rights = if property == Property::GpuPriority {
            PROCESS_QUERY_INFORMATION
        } else {
            0
        };
        let h = self.checked(id, rights)?;
        match property {
            Property::GpuPriority => {
                let mut value = 0;
                let status = unsafe { D3DKMTGetProcessSchedulingPriorityClass(h.0, &mut value) };
                if status != 0 || !(0..=5).contains(&value) {
                    return Err(Fault::new(
                        FaultKind::Unsupported,
                        format!("GPU scheduling query unavailable (NTSTATUS {status:#x})."),
                    ));
                }
                Ok(Value::GpuPriority(value))
            }
            Property::CpuPriority => {
                let value = unsafe { GetPriorityClass(h.0) };
                if value == 0 {
                    return Err(last_fault("GetPriorityClass"));
                }
                Ok(Value::CpuPriority(value))
            }
            Property::EcoQos => {
                let mut value = PROCESS_POWER_THROTTLING_STATE {
                    Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
                    ControlMask: 0,
                    StateMask: 0,
                };
                if unsafe {
                    GetProcessInformation(
                        h.0,
                        ProcessPowerThrottling,
                        (&mut value as *mut PROCESS_POWER_THROTTLING_STATE).cast(),
                        size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
                    )
                } == 0
                {
                    return Err(last_fault(
                        "EcoQoS query unsupported or denied; left untouched",
                    ));
                }
                if value.Version != PROCESS_POWER_THROTTLING_CURRENT_VERSION {
                    return Err(Fault::new(
                        FaultKind::Unsupported,
                        "Unknown EcoQoS structure version; left untouched.",
                    ));
                }
                Ok(Value::EcoQos {
                    control: value.ControlMask,
                    state: value.StateMask,
                })
            }
            Property::MemoryPriority => {
                let mut value = MEMORY_PRIORITY_INFORMATION { MemoryPriority: 0 };
                if unsafe {
                    GetProcessInformation(
                        h.0,
                        ProcessMemoryPriority,
                        (&mut value as *mut MEMORY_PRIORITY_INFORMATION).cast(),
                        size_of::<MEMORY_PRIORITY_INFORMATION>() as u32,
                    )
                } == 0
                {
                    return Err(last_fault("Memory priority query"));
                }
                Ok(Value::MemoryPriority(value.MemoryPriority))
            }
        }
    }
    fn write(&mut self, id: &Identity, value: &Value) -> NativeResult<()> {
        if !value.valid() {
            return Err(Fault::new(
                FaultKind::Other,
                "Invalid native policy value; no setter was called.",
            ));
        }
        if self.cancelled() {
            return Err(Fault::new(
                FaultKind::Denied,
                "Cancelled before native setter.",
            ));
        }
        if self.manual_mode {
            self.authorize(id)?;
            if super::manual::authorize_current_rule(id)? != crate::manual::RuleAction::ReduceLoad {
                return Err(Fault::new(
                    FaultKind::Denied,
                    "No saved permission for scheduling changes.",
                ));
            }
        }
        let h = self.checked(id, PROCESS_SET_INFORMATION)?;
        let ok = match *value {
            Value::GpuPriority(priority) => {
                if !(0..=5).contains(&priority) {
                    return Err(Fault::new(
                        FaultKind::Other,
                        "Invalid GPU priority in journal.",
                    ));
                }
                let status = unsafe { D3DKMTSetProcessSchedulingPriorityClass(h.0, priority) };
                if status != 0 {
                    return Err(Fault::new(
                        FaultKind::Unsupported,
                        format!("GPU priority setter unavailable (NTSTATUS {status:#x})."),
                    ));
                }
                1
            }
            Value::CpuPriority(priority) => unsafe { SetPriorityClass(h.0, priority) },
            Value::EcoQos { control, state } => {
                let v = PROCESS_POWER_THROTTLING_STATE {
                    Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
                    ControlMask: control,
                    StateMask: state,
                };
                unsafe {
                    SetProcessInformation(
                        h.0,
                        ProcessPowerThrottling,
                        (&v as *const PROCESS_POWER_THROTTLING_STATE).cast(),
                        size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
                    )
                }
            }
            Value::MemoryPriority(priority) => {
                let v = MEMORY_PRIORITY_INFORMATION {
                    MemoryPriority: priority,
                };
                unsafe {
                    SetProcessInformation(
                        h.0,
                        ProcessMemoryPriority,
                        (&v as *const MEMORY_PRIORITY_INFORMATION).cast(),
                        size_of::<MEMORY_PRIORITY_INFORMATION>() as u32,
                    )
                }
            }
        };
        if ok == 0 {
            return Err(last_fault("Priority write"));
        }
        Ok(())
    }
    fn close(&mut self, id: &Identity, force: bool, timeout_ms: u32) -> NativeResult<CloseState> {
        self.authorize(id)?;
        if self.manual_mode
            && (force
                || super::manual::authorize_current_rule(id)? != crate::manual::RuleAction::Close)
        {
            return Err(Fault::new(
                FaultKind::Denied,
                "Manual Game Mode permits only the approved normal-close operation.",
            ));
        }
        if force {
            // Direct force is a distinct reviewed action, never a WM_CLOSE fallback.
            let h = self.checked(id, PROCESS_TERMINATE)?;
            if self.cancelled() {
                return Ok(CloseState::KeptOpen);
            }
            if unsafe { TerminateProcess(h.0, 1) } == 0 {
                return Err(last_fault("TerminateProcess"));
            }
            return if unsafe { WaitForSingleObject(h.0, 3000) } == WAIT_OBJECT_0 {
                Ok(CloseState::Terminated)
            } else {
                Err(Fault::new(
                    FaultKind::Other,
                    "Termination requested; exit remains unconfirmed.",
                ))
            };
        }
        let h = self.checked(id, 0)?;
        struct Context {
            pid: u32,
            windows: Vec<HWND>,
        }
        unsafe extern "system" fn callback(window: HWND, data: LPARAM) -> i32 {
            let ctx = &mut *(data as *mut Context);
            let mut pid = 0;
            GetWindowThreadProcessId(window, &mut pid);
            if pid == ctx.pid && ctx.windows.len() < 16 {
                ctx.windows.push(window);
            }
            i32::from(ctx.windows.len() < 16)
        }
        let mut context = Context {
            pid: id.pid,
            windows: Vec::new(),
        };
        unsafe {
            EnumWindows(Some(callback), (&mut context as *mut Context) as LPARAM);
        }
        if context.windows.is_empty() {
            return Ok(CloseState::KeptOpen);
        }
        let mut requested = false;
        for window in context.windows {
            if self.cancelled() {
                break;
            }
            let mut owner = 0;
            unsafe {
                GetWindowThreadProcessId(window, &mut owner);
            }
            if owner != id.pid || unsafe { WaitForSingleObject(h.0, 0) } == WAIT_OBJECT_0 {
                continue;
            }
            // Timeout may mean delivery already occurred: never mark it safely undone.
            requested = true;
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::SendMessageTimeoutW(
                    window,
                    WM_CLOSE,
                    0,
                    0,
                    0x0002 | 0x0020,
                    1000,
                    null_mut(),
                );
            }
        }
        let deadline =
            Instant::now() + Duration::from_millis(u64::from(timeout_ms.clamp(1000, 15000)));
        while Instant::now() < deadline {
            if unsafe { WaitForSingleObject(h.0, 100) } == WAIT_OBJECT_0 {
                return Ok(CloseState::ClosedGracefully);
            }
            if self.cancelled() {
                break;
            }
        }
        Ok(if requested {
            CloseState::RequestedPending
        } else {
            CloseState::KeptOpen
        })
    }
}
