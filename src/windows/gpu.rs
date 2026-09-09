use crate::{gpu::{parse_instance, valid_percent}, model::*, policy};
use super::{process, wide};
use std::{collections::BTreeMap, mem::size_of, ptr::null, thread, time::Duration};

// PDH ABI from pdh.h. Aligned backing buffers keep native pointers valid until copied.
#[repr(C)] union CounterNumber { double_value: f64, large_value: i64 }
#[repr(C)] struct CounterValue { status: u32, number: CounterNumber }
#[repr(C)] struct CounterItem { name: *const u16, value: CounterValue }
#[link(name = "pdh")]
unsafe extern "system" {
    fn PdhOpenQueryW(source: *const u16, data: usize, query: *mut isize) -> u32;
    fn PdhAddEnglishCounterW(query: isize, path: *const u16, data: usize, counter: *mut isize) -> u32;
    fn PdhCollectQueryData(query: isize) -> u32;
    fn PdhGetFormattedCounterArrayW(counter: isize, format: u32, bytes: *mut u32, count: *mut u32, buffer: *mut CounterItem) -> u32;
    fn PdhCloseQuery(query: isize) -> u32;
}

const MORE_DATA: u32 = 0x800007d2;
const DOUBLE: u32 = 0x200;
const LARGE: u32 = 0x400;
struct Query(isize);
impl Drop for Query { fn drop(&mut self) { unsafe { PdhCloseQuery(self.0); } } }

fn values(counter: isize, format: u32) -> AppResult<Vec<(String, f64)>> {
    for _ in 0..3 {
        let (mut bytes, mut count) = (0, 0);
        let status = unsafe { PdhGetFormattedCounterArrayW(counter, format, &mut bytes, &mut count, std::ptr::null_mut()) };
        if status != MORE_DATA { return Err(format!("Counter data unavailable: {status:#x}")); }
        if bytes == 0 || bytes > 64 * 1024 * 1024 { return Err("Invalid PDH buffer size.".into()); }
        let mut data = vec![0u64; (bytes as usize).div_ceil(8)];
        let capacity = data.len() * 8;
        let status = unsafe { PdhGetFormattedCounterArrayW(counter, format, &mut bytes, &mut count, data.as_mut_ptr().cast()) };
        if status == MORE_DATA { continue; }
        if status != 0 { return Err(format!("Counter sample unavailable: {status:#x}")); }
        if count as usize > capacity / size_of::<CounterItem>() { return Err("Invalid PDH item count.".into()); }
        let start = data.as_ptr() as usize;
        let end = start + capacity;
        let items = unsafe { std::slice::from_raw_parts(data.as_ptr().cast::<CounterItem>(), count as usize) };
        let mut result = Vec::new();
        for item in items {
            if item.value.status > 1 { continue; }
            let address = item.name as usize;
            if address < start || address >= end || address % 2 != 0 { continue; }
            let max_units = ((end - address) / 2).min(4096);
            let units = unsafe { std::slice::from_raw_parts(item.name, max_units) };
            let Some(n) = units.iter().position(|v| *v == 0) else { continue; };
            let number = unsafe { if format == LARGE { item.value.number.large_value as f64 } else { item.value.number.double_value } };
            if number.is_finite() && number >= 0.0 { result.push((String::from_utf16_lossy(&units[..n]), number)); }
        }
        return Ok(result);
    }
    Err("GPU process set changed repeatedly; refresh the sample.".into())
}

pub fn snapshot() -> AppResult<Snapshot> {
    let before = process::enumerate().map_err(|e| e.message)?;
    let mut query = 0;
    let status = unsafe { PdhOpenQueryW(null(), 0, &mut query) };
    if status != 0 { return Ok(Snapshot { processes: before, gpu_status: format!("GPU counters unavailable ({status:#x}); no GPU usage is assumed to be zero.") }); }
    let query = Query(query);
    let paths = [r"\GPU Engine(*)\Utilization Percentage", r"\GPU Process Memory(*)\Dedicated Usage", r"\GPU Process Memory(*)\Shared Usage"];
    let mut handles = Vec::new();
    let mut notices = Vec::new();
    for path in paths {
        let mut h = 0;
        let status = unsafe { PdhAddEnglishCounterW(query.0, wide(path).as_ptr(), 0, &mut h) };
        if status == 0 { handles.push(Some(h)); } else { handles.push(None); notices.push(format!("{path}: {status:#x}")); }
    }
    let first = unsafe { PdhCollectQueryData(query.0) };
    thread::sleep(Duration::from_millis(1100));
    let second = unsafe { PdhCollectQueryData(query.0) };
    if first != 0 || second != 0 { notices.push(format!("Collection status: {first:#x}/{second:#x}.")); }
    let mut readings: BTreeMap<u32, BTreeMap<(String, String), GpuReading>> = BTreeMap::new();
    for (kind, counter) in handles.into_iter().enumerate() {
        let Some(counter) = counter else { continue; };
        let samples = match values(counter, if kind == 0 { DOUBLE } else { LARGE }) {
            Ok(v) => v, Err(e) => { notices.push(e); continue; }
        };
        for (name, value) in samples {
            let Some(instance) = parse_instance(&name) else { continue; };
            let engine = instance.engine.unwrap_or_else(|| "memory".into());
            let row = readings.entry(instance.pid).or_default().entry((instance.adapter.clone(), engine.clone())).or_insert_with(|| GpuReading { adapter: instance.adapter.clone(), engine, ..Default::default() });
            match kind {
                0 => row.percent = valid_percent(value),
                1 => row.dedicated_bytes = Some(value as u64),
                _ => row.shared_bytes = Some(value as u64),
            }
        }
    }
    let mut after = process::enumerate().map_err(|e| e.message)?;
    for row in &mut after {
        if before.iter().any(|p| policy::same_process(&p.identity, &row.identity) && p.identity.created != 0) {
            if let Some(values) = readings.remove(&row.identity.pid) { row.gpu = values.into_values().collect(); }
        }
    }
    after.sort_by(|a, b| {
        let x = crate::gpu::busiest_engine(&a.gpu).unwrap_or(-1.0);
        let y = crate::gpu::busiest_engine(&b.gpu).unwrap_or(-1.0);
        y.total_cmp(&x).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())).then(a.identity.pid.cmp(&b.identity.pid))
    });
    let status = if notices.is_empty() {
        "Sampled WDDM counters. GPU% = busiest engine, not sum; memory = dedicated allocation accounting, not exclusive VRAM or a reclaim promise.".into()
    } else { format!("Some GPU measurements unavailable: {}", notices.join(" | ")) };
    Ok(Snapshot { processes: after, gpu_status: status })
}
