use super::*;
#[test]
fn service_findings_do_not_invent_a_healthy_result() {
    let healthy = summarize(
        Job::Failed,
        &Ok("UNIT LOAD ACTIVE SUB DESCRIPTION\n0 loaded units listed.\n".into()),
    );
    assert_eq!(healthy.tone, "success");
    assert_eq!(healthy.facts[0].value, "0");
    let failed = summarize(
        Job::Failed,
        &Ok("example.service loaded failed failed Example\n".into()),
    );
    assert_eq!(failed.tone, "warning");
    assert_eq!(failed.facts[0].value, "1");
    assert_eq!(failed.facts[1].value, "example.service");
    for text in [
        "",
        "Failed to connect to bus",
        "Unexpected output",
        "error failed to connect to bus",
    ] {
        let result = summarize(Job::Failed, &Ok(text.into()));
        assert_eq!(result.tone, "warning");
        assert_eq!(result.facts[0].value, "Not reported");
    }
}
#[test]
fn startup_duration_keeps_multi_part_units() {
    let result = summarize(
        Job::Boot,
        &Ok("1min 4.2s example.service\n300ms next.service\n".into()),
    );
    assert_eq!(result.facts[0].value, "example.service");
    assert_eq!(result.facts[1].value, "1min 4.2s");
    assert_eq!(result.facts[2].value, "2");
}
#[test]
fn vulkan_uses_first_reported_device() {
    let result=summarize(Job::Vulkan,&Ok("deviceName = First GPU\napiVersion = 1.3.0\ndriverName = driver\ndeviceType = DISCRETE_GPU\ndeviceName = Second GPU\n".into()));
    assert_eq!(result.facts[0].value, "First GPU");
    assert_eq!(result.facts[2].value, "driver");
    assert_eq!(summarize(Job::Vulkan, &Ok("".into())).tone, "warning");
}
#[test]
fn report_flags_failed_units_without_losing_usage() {
    let result=summarize(Job::Report,&Ok("Memory: 4 GiB / 16 GiB\nRoot storage: 20 GiB / 100 GiB\nInstalled packages: 300\nFailed services\nexample.service loaded failed failed Example\n".into()));
    assert_eq!(result.tone, "warning");
    assert_eq!(result.facts[0].value, "4 GiB / 16 GiB");
    assert_eq!(result.facts[3].value, "1");
}
#[test]
fn command_errors_never_show_success_facts() {
    let result = summarize(Job::Vulkan, &Err("missing command".into()));
    assert_eq!(result.tone, "error");
    assert_eq!(result.facts.len(), 1);
}
