use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::OnceLock;
use rtrb::{RingBuffer, Producer, Consumer};

pub struct InstanceSharedData {
    pub instance_id: usize,
    pub name: Arc<Mutex<String>>,
    pub dry_buffer_tx: Arc<Mutex<Option<Producer<f32>>>>,
    pub dry_buffer_rx: Arc<Mutex<Option<Consumer<f32>>>>,
    pub is_sender: AtomicBool,
    pub is_receiver: AtomicBool,
    pub ab_mode: AtomicU32,
    pub sweep_active: AtomicBool,
    pub sweep_trigger: AtomicBool,
    pub sweep_progress: AtomicU32,
    pub sweep_sample_rate: AtomicU32,
}

impl InstanceSharedData {
    pub fn new(instance_id: usize) -> Self {
        let (tx, rx) = RingBuffer::new(16384);
        Self {
            instance_id,
            name: Arc::new(Mutex::new(String::new())),
            dry_buffer_tx: Arc::new(Mutex::new(Some(tx))),
            dry_buffer_rx: Arc::new(Mutex::new(Some(rx))),
            is_sender: AtomicBool::new(false),
            is_receiver: AtomicBool::new(false),
            ab_mode: AtomicU32::new(0),
            sweep_active: AtomicBool::new(false),
            sweep_trigger: AtomicBool::new(false),
            sweep_progress: AtomicU32::new(0),
            sweep_sample_rate: AtomicU32::new(48000),
        }
    }
}

pub struct InstanceRegistry {
    pub instances: HashMap<usize, Arc<InstanceSharedData>>,
    next_id: usize,
}

impl InstanceRegistry {
    fn new() -> Self {
        Self {
            instances: HashMap::new(),
            next_id: 0,
        }
    }
}

static REGISTRY: OnceLock<Mutex<InstanceRegistry>> = OnceLock::new();

pub fn get_registry() -> &'static Mutex<InstanceRegistry> {
    REGISTRY.get_or_init(|| Mutex::new(InstanceRegistry::new()))
}

pub fn next_instance_id() -> usize {
    let mut reg = get_registry().lock().unwrap();
    let id = reg.next_id;
    reg.next_id += 1;
    id
}

pub type RtSnapshotInfo = Vec<serde_json::Value>;
pub type RtMacroMappings = Vec<serde_json::Value>;

pub fn parse_snapshots(json: &str) -> RtSnapshotInfo {
    serde_json::from_str(json).unwrap_or_default()
}

pub fn parse_macro_mappings(json: &str) -> RtMacroMappings {
    serde_json::from_str(json).unwrap_or_default()
}
