use crate::jobs::Job;

pub(crate) struct Finding {
    pub caption: &'static str,
    pub value: String,
    pub icon: &'static str,
}
pub(crate) struct Findings {
    pub title: &'static str,
    pub detail: &'static str,
    pub tone: &'static str,
    pub facts: Vec<Finding>,
}
fn field(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (key, value) = line.trim().split_once(['=', ':'])?;
        (key.trim() == name && !value.trim().is_empty()).then(|| value.trim().into())
    })
}
fn failed_units(text: &str) -> Option<usize> {
    let count = text
        .lines()
        .filter(|line| {
            let words: Vec<_> = line.split_whitespace().collect();
            words.len() >= 4
                && words.contains(&"failed")
                && words.first().is_some_and(|name| {
                    [
                        ".service",
                        ".mount",
                        ".socket",
                        ".target",
                        ".timer",
                        ".path",
                        ".device",
                        ".slice",
                        ".scope",
                        ".automount",
                        ".swap",
                    ]
                    .iter()
                    .any(|suffix| name.ends_with(suffix))
                })
        })
        .count();
    if count > 0 {
        Some(count)
    } else if text.contains("0 loaded units listed") || text.contains("0 loaded units") {
        Some(0)
    } else {
        None
    }
}
fn fact(caption: &'static str, value: Option<String>, icon: &'static str) -> Finding {
    Finding {
        caption,
        value: value.unwrap_or_else(|| "Not reported".into()),
        icon,
    }
}
pub(crate) fn summarize(job: Job, result: &Result<String, String>) -> Findings {
    let text = match result {
        Ok(text) => text,
        Err(_) => {
            return Findings {
                title: "Check needs attention",
                detail: "The command did not complete successfully. Read the highlighted output for the error.",
                tone: "error",
                facts: vec![fact(
                    "Result",
                    Some("Command error".into()),
                    "dialog-error-symbolic",
                )],
            };
        }
    };
    let mut summary = Findings {
        title: "Check completed",
        detail: "Values below come from this check's output.",
        tone: "success",
        facts: Vec::new(),
    };
    match job {
        Job::Failed => {
            let count = failed_units(text);
            summary.title = if count == Some(0) {
                "No failed services reported"
            } else if count.is_some() {
                "Services need attention"
            } else {
                "Review service results"
            };
            summary.tone = if count == Some(0) {
                "success"
            } else {
                "warning"
            };
            summary.detail =
                "This checks system units in the current boot. It does not restart or repair them.";
            summary.facts.push(fact(
                "Failed units",
                count.map(|v| v.to_string()),
                "dialog-warning-symbolic",
            ));
            let names: Vec<_> = text
                .lines()
                .filter(|l| l.split_whitespace().any(|w| w == "failed"))
                .filter_map(|l| {
                    l.split_whitespace().find(|w| {
                        w.ends_with(".service") || w.ends_with(".mount") || w.ends_with(".socket")
                    })
                })
                .collect();
            summary.facts.push(fact(
                "First affected unit",
                names
                    .first()
                    .map(|v| v.to_string())
                    .or_else(|| (count == Some(0)).then(|| "None".into())),
                "preferences-system-symbolic",
            ));
        }
        Job::Boot => {
            summary.title = "Startup timings collected";
            summary.detail = "Units start in parallel. These durations are not the total boot time or a diagnosis of a fault.";
            let units: Vec<_> = text
                .lines()
                .filter_map(|l| {
                    let (duration, unit) = l.trim().rsplit_once(char::is_whitespace)?;
                    (duration.chars().next()?.is_ascii_digit() && unit.contains('.'))
                        .then_some((duration, unit))
                })
                .collect();
            summary.facts.push(fact(
                "Slowest startup unit",
                units.first().map(|(_, u)| (*u).into()),
                "appointment-soon-symbolic",
            ));
            summary.facts.push(fact(
                "Startup duration",
                units.first().map(|(d, _)| (*d).into()),
                "document-open-recent-symbolic",
            ));
            summary.facts.push(fact(
                "Timed units",
                (!units.is_empty()).then(|| units.len().to_string()),
                "view-list-symbolic",
            ));
        }
        Job::Vulkan => {
            summary.title = "Vulkan devices inspected";
            summary.detail = "The first reported device is shown here. The full output includes other GPUs and driver details.";
            for (caption, key, icon) in [
                ("Graphics device", "deviceName", "video-display-symbolic"),
                (
                    "Vulkan API",
                    "apiVersion",
                    "applications-engineering-symbolic",
                ),
                ("Driver", "driverName", "preferences-system-symbolic"),
                ("Device type", "deviceType", "computer-symbolic"),
            ] {
                summary.facts.push(fact(caption, field(text, key), icon));
            }
            if field(text, "deviceName").is_none() {
                summary.title = "No GPU details reported";
                summary.tone = "warning";
            }
        }
        Job::Report => {
            summary.title = "System report ready";
            summary.detail =
                "A snapshot of this system. Review the full report before copying or sharing it.";
            for (caption, key, icon) in [
                ("Memory in use", "Memory", "media-flash-symbolic"),
                ("Root storage", "Root storage", "drive-harddisk-symbolic"),
                (
                    "Installed packages",
                    "Installed packages",
                    "package-x-generic-symbolic",
                ),
            ] {
                summary.facts.push(fact(caption, field(text, key), icon));
            }
            let count = text
                .split_once("Failed services\n")
                .and_then(|(_, services)| failed_units(services));
            summary.facts.push(fact(
                "Failed system units",
                count.map(|v| v.to_string()),
                "preferences-system-symbolic",
            ));
            if count.is_some_and(|v| v > 0) {
                summary.tone = "warning";
                summary.title = "Report ready · failed units found";
            }
        }
        _ => {}
    }
    summary
}

#[cfg(test)]
#[path = "diagnostic_results_tests.rs"]
mod tests;
