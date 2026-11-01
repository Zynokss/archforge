use std::{
    collections::HashMap,
    ffi::CString,
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default)]
pub struct Package {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Default)]
pub struct Inventory {
    pub packages: Vec<Package>,
    pub error: Option<String>,
    pub cpu: String,
    pub gpu: String,
    pub kernel: String,
    pub hostname: String,
    pub desktop: String,
    pub failed_services: Option<usize>,
}

impl Inventory {
    pub fn load() -> Self {
        let result = run("/usr/bin/pacman", &["-Q"], 20);
        let (packages, error) = match result {
            Ok(output) => (parse_packages(&output), None),
            Err(error) => (vec![], Some(error)),
        };
        let cpu = read("/proc/cpuinfo")
            .lines()
            .find_map(|l| {
                l.strip_prefix("model name")
                    .and_then(|s| s.split_once(':'))
                    .map(|(_, s)| s.trim().to_string())
            })
            .unwrap_or_else(|| "Unknown processor".into());
        let pci = run("/usr/bin/lspci", &[], 5).unwrap_or_default();
        let gpu = pci
            .lines()
            .filter(|l| {
                l.contains("VGA compatible")
                    || l.contains("3D controller")
                    || l.contains("Display controller")
            })
            .map(|l| l.split_once(": ").map(|(_, s)| s).unwrap_or(l))
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            packages,
            error,
            cpu,
            gpu: if gpu.is_empty() {
                "GPU information unavailable (requires pciutils)".into()
            } else {
                gpu
            },
            kernel: read("/proc/sys/kernel/osrelease").trim().into(),
            hostname: read("/proc/sys/kernel/hostname").trim().into(),
            desktop: std::env::var("XDG_CURRENT_DESKTOP")
                .unwrap_or_else(|_| "Unknown session".into()),
            failed_services: run(
                "/usr/bin/systemctl",
                &["--failed", "--no-legend", "--plain", "--no-pager"],
                5,
            )
            .ok()
            .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count()),
        }
    }
    pub fn unmanaged_tool(&self, name: &str) -> bool {
        use std::os::unix::fs::PermissionsExt;
        let binary = match name {
            "steam" => "steam",
            "gamemode" => "gamemoderun",
            "mangohud" => "mangohud",
            "vulkan-tools" => "vulkaninfo",
            "gamescope" => "gamescope",
            "lutris" => "lutris",
            _ => return false,
        };
        !self.installed(name)
            && std::fs::metadata(format!("/usr/bin/{binary}"))
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    pub fn installed(&self, name: &str) -> bool {
        self.packages.iter().any(|p| p.name == name)
    }
}

pub fn parse_packages(output: &str) -> Vec<Package> {
    output
        .lines()
        .filter_map(|line| {
            line.split_once(' ').map(|(name, version)| Package {
                name: name.into(),
                version: version.into(),
            })
        })
        .collect()
}

pub fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

pub fn is_arch(contents: &str) -> bool {
    contents.lines().any(|l| {
        l.trim()
            .strip_prefix("ID=")
            .is_some_and(|id| id.trim_matches('"') == "arch")
    })
}

#[derive(Default)]
pub struct Sampler {
    previous: Option<(u64, u64)>,
}

#[derive(Default, Clone)]
pub struct Metrics {
    pub cpu: Option<f64>,
    pub memory: f64,
    pub used_memory: u64,
    pub total_memory: u64,
    pub used_disk: u64,
    pub total_disk: u64,
    pub uptime: u64,
    pub temperature: Option<f64>,
}

pub fn cpu_ticks(contents: &str) -> Option<(u64, u64)> {
    let v: Vec<u64> = contents
        .lines()
        .next()?
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    if v.len() < 4 {
        return None;
    }
    Some((v.iter().sum(), v[3] + v.get(4).copied().unwrap_or(0)))
}

pub fn memory_bytes(contents: &str) -> (u64, u64) {
    let map: HashMap<&str, u64> = contents
        .lines()
        .filter_map(|l| {
            let (key, rest) = l.split_once(':')?;
            Some((
                key,
                rest.split_whitespace().next()?.parse::<u64>().ok()? * 1024,
            ))
        })
        .collect();
    let total = map.get("MemTotal").copied().unwrap_or(0);
    (
        total.saturating_sub(map.get("MemAvailable").copied().unwrap_or(total)),
        total,
    )
}

impl Sampler {
    pub fn sample(&mut self) -> Metrics {
        let cpu = cpu_ticks(&read("/proc/stat")).and_then(|next| {
            let prev = self.previous.replace(next)?;
            let total = next.0.saturating_sub(prev.0);
            (total > 0).then(|| 100.0 * (1.0 - next.1.saturating_sub(prev.1) as f64 / total as f64))
        });
        let (used_memory, total_memory) = memory_bytes(&read("/proc/meminfo"));
        let (used_disk, total_disk) = disk_usage("/").unwrap_or_default();
        let uptime = read("/proc/uptime")
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0) as u64;
        // Only report CPU package sensors, never an arbitrary thermal zone.
        let temperature = fs::read_dir("/sys/class/thermal")
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .find_map(|entry| {
                let kind = fs::read_to_string(entry.path().join("type")).ok()?;
                if !matches!(kind.trim(), "x86_pkg_temp" | "cpu-thermal" | "cpu_thermal") {
                    return None;
                }
                let value = fs::read_to_string(entry.path().join("temp"))
                    .ok()?
                    .trim()
                    .parse::<f64>()
                    .ok()?
                    / 1000.0;
                (0.0..150.0).contains(&value).then_some(value)
            })
            .or_else(|| {
                fs::read_dir("/sys/class/hwmon")
                    .ok()?
                    .flatten()
                    .find_map(|entry| {
                        let name = fs::read_to_string(entry.path().join("name")).ok()?;
                        if !matches!(name.trim(), "k10temp" | "coretemp" | "zenpower") {
                            return None;
                        }
                        let value = fs::read_to_string(entry.path().join("temp1_input"))
                            .ok()?
                            .trim()
                            .parse::<f64>()
                            .ok()?
                            / 1000.0;
                        (0.0..150.0).contains(&value).then_some(value)
                    })
            });
        Metrics {
            cpu,
            memory: if total_memory > 0 {
                used_memory as f64 / total_memory as f64 * 100.0
            } else {
                0.0
            },
            used_memory,
            total_memory,
            used_disk,
            total_disk,
            uptime,
            temperature,
        }
    }
}

pub fn disk_usage(path: &str) -> Option<(u64, u64)> {
    let path = CString::new(path).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: path is nul-terminated and stat is writable; only read after success.
    if unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
        return None;
    }
    let stat = unsafe { stat.assume_init() };
    Some((
        (stat.f_blocks - stat.f_bfree) * stat.f_frsize,
        stat.f_blocks * stat.f_frsize,
    ))
}

pub fn bytes(value: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = value as f64;
    let mut i = 0;
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1} {}", units[i])
}

struct CapturedOutput {
    text: String,
    complete: bool,
}

fn capture_output(
    mut stream: impl std::io::Read + std::os::fd::AsRawFd,
    limit: usize,
    deadline: Instant,
) -> CapturedOutput {
    let fd = stream.as_raw_fd();
    // SAFETY: this is the exclusively owned, valid output pipe of our child.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return CapturedOutput {
            text: "Could not configure the command output pipe.".into(),
            complete: false,
        };
    }
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    let mut truncated = false;
    let complete = loop {
        if Instant::now() >= deadline {
            break false;
        }
        match stream.read(&mut chunk) {
            Ok(0) => break true,
            Ok(count) => {
                let keep = count.min(limit.saturating_sub(bytes.len()));
                bytes.extend_from_slice(&chunk[..keep]);
                truncated |= keep < count;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break false,
        }
    };
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        text.push_str("\n[Output truncated; the remaining output was drained.]\n");
    }
    CapturedOutput { text, complete }
}

fn kill_command_group(child: &mut std::process::Child) {
    // SAFETY: the child starts a new process group with its own PID. A negative
    // PID targets only that group, never the application's or the desktop's.
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.try_wait();
}

/// Bound the complete command, including inherited pipes and helper processes.
/// Output is capped in memory while both streams continue to drain.
pub fn run(program: &str, args: &[&str], seconds: u64) -> Result<String, String> {
    use std::os::unix::process::CommandExt;
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .process_group(0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot start {program}: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || capture_output(stdout, 4 * 1024 * 1024, deadline));
    let err = thread::spawn(move || capture_output(stderr, 1024 * 1024, deadline));
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                kill_command_group(&mut child);
                break None;
            }
        }
    };
    let fallback = || CapturedOutput {
        text: "Output reader failed.".into(),
        complete: false,
    };
    let output = out.join().unwrap_or_else(|_| fallback());
    let error = err.join().unwrap_or_else(|_| fallback());
    if status.is_none() || !output.complete || !error.complete {
        kill_command_group(&mut child);
        // Reap without blocking the result if a privileged helper or a process
        // in kernel I/O wait cannot be terminated by the desktop user.
        let operation = (program == "/usr/bin/pkexec").then(crate::Operation::new);
        thread::spawn(move || {
            let _operation = operation;
            let _ = child.wait();
        });
        return Err(format!(
            "{program} timed out or its output could not be fully read within {seconds}s. An authenticated operation may still be running; review the system before retrying.\n{}{}",
            output.text, error.text
        ));
    }
    let status = status.unwrap();
    if status.success() {
        Ok(format!("{}{}", output.text, error.text))
    } else {
        Err(format!(
            "Exit {}\n{}{}",
            status
                .code()
                .map_or_else(|| "signal".into(), |n| n.to_string()),
            output.text,
            error.text
        ))
    }
}
