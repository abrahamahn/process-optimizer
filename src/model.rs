use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
pub type AppResult<T> = Result<T, String>;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Identity {
    pub pid: u32,
    pub created: u64,
    pub path: String,
    pub session_id: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessRow {
    pub identity: Identity,
    pub parent_pid: u32,
    pub name: String,
    pub protected_reason: Option<String>,
    pub gpu: Vec<GpuReading>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GpuReading {
    pub adapter: String,
    pub engine: String,
    pub percent: Option<f64>,
    pub dedicated_bytes: Option<u64>,
    pub shared_bytes: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub processes: Vec<ProcessRow>,
    pub gpu_status: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    LowerPriorities,
    Close,
    ForceClose,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovedAction {
    pub target: Identity,
    pub action: ActionKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Options {
    pub gpu_priority: bool,
    pub cpu_priority: bool,
    pub eco_qos: bool,
    pub memory_priority: bool,
    pub launch_game: bool,
    pub close_timeout_ms: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            gpu_priority: true,
            cpu_priority: false,
            eco_qos: false,
            memory_priority: false,
            launch_game: false,
            close_timeout_ms: 4000,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    pub game_path: String,
    pub protected_paths: Vec<String>,
    pub options: Options,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub game_path: String,
    pub game: Option<Identity>,
    pub actions: Vec<ApprovedAction>,
    pub protected_paths: Vec<String>,
    pub options: Options,
    pub consent: bool,
    pub force_consent: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Property {
    GpuPriority,
    CpuPriority,
    EcoQos,
    MemoryPriority,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    GpuPriority(i32),
    CpuPriority(u32),
    EcoQos { control: u32, state: u32 },
    MemoryPriority(u32),
}

impl Value {
    pub fn property(&self) -> Property {
        match self {
            Self::GpuPriority(_) => Property::GpuPriority,
            Self::CpuPriority(_) => Property::CpuPriority,
            Self::EcoQos { .. } => Property::EcoQos,
            Self::MemoryPriority(_) => Property::MemoryPriority,
        }
    }

    /// Never raise a background application's existing priority.
    pub fn background(&self) -> Self {
        match *self {
            Self::GpuPriority(v) => Self::GpuPriority(v.min(1)),
            Self::CpuPriority(v) => Self::CpuPriority(if v == 0x40 || v == 0x4000 { v } else { 0x4000 }),
            Self::EcoQos { control, state } => Self::EcoQos { control: control | 1, state: state | 1 },
            Self::MemoryPriority(v) => Self::MemoryPriority(v.min(2)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeState {
    Prepared,
    Applied,
    RestorePrepared,
    Restored,
    ProcessGone,
    Conflict,
    UserKept,
}

impl ChangeState {
    pub fn resolved(self) -> bool {
        matches!(self, Self::Restored | Self::ProcessGone | Self::UserKept)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    pub target: Identity,
    pub before: Value,
    pub applied: Value,
    pub state: ChangeState,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloseState {
    IntentRecorded,
    ClosedGracefully,
    Terminated,
    KeptOpen,
    AlreadyGone,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloseRecord {
    pub target: Identity,
    pub force_allowed: bool,
    pub state: CloseState,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    Pending,
    Preparing,
    Active,
    Restoring,
    Restored,
    RecoveryNeeded,
    UserAcknowledged,
}

impl Stage {
    pub fn finished(self) -> bool {
        matches!(self, Self::Restored | Self::UserAcknowledged)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub schema: u32,
    pub id: String,
    pub stage: Stage,
    pub plan: Plan,
    pub worker: Option<Identity>,
    pub game: Option<Identity>,
    pub changes: Vec<Change>,
    pub closed: Vec<CloseRecord>,
    pub events: Vec<String>,
}

impl Session {
    pub fn new(id: String, plan: Plan) -> Self {
        Self {
            schema: SCHEMA_VERSION,
            id,
            stage: Stage::Pending,
            plan,
            worker: None,
            game: None,
            changes: vec![],
            closed: vec![],
            events: vec![],
        }
    }
    pub fn note(&mut self, message: impl Into<String>) {
        if self.events.len() < 2048 {
            self.events.push(message.into());
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultKind { Gone, Denied, Unsupported, Other }

#[derive(Clone, Debug)]
pub struct Fault {
    pub kind: FaultKind,
    pub message: String,
}

impl Fault {
    pub fn new(kind: FaultKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into() }
    }
}

pub type NativeResult<T> = Result<T, Fault>;
