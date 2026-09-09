//! PDH instance parsing is pure; missing or invalid measurements are not zero.
use crate::model::GpuReading;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instance {
    pub pid: u32,
    pub adapter: String,
    pub engine: Option<String>,
}

pub fn parse_instance(name: &str) -> Option<Instance> {
    let pieces: Vec<_> = name.split('_').collect();
    if pieces.first().copied()? != "pid" { return None; }
    let pid = pieces.get(1)?.parse().ok()?;
    let luid = pieces.iter().position(|v| *v == "luid")?;
    let hi = pieces.get(luid + 1)?;
    let lo = pieces.get(luid + 2)?;
    if !hi.starts_with("0x") || !lo.starts_with("0x") { return None; }
    u32::from_str_radix(&hi[2..], 16).ok()?;
    u32::from_str_radix(&lo[2..], 16).ok()?;
    let phys = pieces.iter().position(|v| *v == "phys").and_then(|i| pieces.get(i + 1)).copied().unwrap_or("0");
    let adapter = format!("{hi}:{lo}/phys{phys}");
    let engine = pieces.iter().position(|v| *v == "eng").and_then(|i| pieces.get(i + 1)).map(|index| {
        let kind = pieces.iter().position(|v| *v == "engtype").map(|i| pieces[i + 1..].join("_")).unwrap_or_default();
        format!("{index}/{kind}")
    });
    Some(Instance { pid, adapter, engine })
}

pub fn valid_percent(value: f64) -> Option<f64> {
    if value.is_finite() && value >= 0.0 { Some(value.min(100.0)) } else { None }
}

pub fn busiest_engine(readings: &[GpuReading]) -> Option<f64> {
    readings.iter().filter_map(|r| r.percent).filter(|v| v.is_finite()).reduce(f64::max)
}

pub fn dedicated_allocations(readings: &[GpuReading]) -> Option<u64> {
    // Dedicated usage is attached once per adapter, not once per engine.
    let mut adapters = BTreeMap::new();
    for r in readings {
        if let Some(bytes) = r.dedicated_bytes { adapters.insert(&r.adapter, bytes); }
    }
    if adapters.is_empty() { None } else { Some(adapters.values().fold(0u64, |a, b| a.saturating_add(*b))) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn parses_adapter_and_engine() {
        let x = parse_instance("pid_42_luid_0x00000000_0x000012AB_phys_0_eng_2_engtype_VideoDecode").unwrap();
        assert_eq!(x.pid, 42); assert_eq!(x.adapter, "0x00000000:0x000012AB/phys0"); assert_eq!(x.engine.unwrap(), "2/VideoDecode");
    }
    #[test] fn invalid_instances_are_ignored() { for s in ["_Total", "pid_x_luid_0x00_0x12", "pid_42_luid_foo_bar", "pid_2"] { assert!(parse_instance(s).is_none()); } }
    #[test] fn malformed_measurements_are_unknown() { assert_eq!(valid_percent(f64::NAN), None); assert_eq!(valid_percent(-1.0), None); assert_eq!(valid_percent(120.0), Some(100.0)); }
    #[test] fn parallel_engines_are_not_summed() {
        let rows = vec![GpuReading { percent: Some(70.0), ..Default::default() }, GpuReading { percent: Some(50.0), ..Default::default() }];
        assert_eq!(busiest_engine(&rows), Some(70.0));
    }
    #[test] fn memory_not_double_counted_across_engines() {
        let r = GpuReading { adapter: "a".into(), dedicated_bytes: Some(100), ..Default::default() };
        assert_eq!(dedicated_allocations(&[r.clone(), r]), Some(100));
        assert_eq!(dedicated_allocations(&[]), None);
    }
}
