use runlane_core::OperatingSystem;

fn main() {
    let os = detect_os();
    println!("runlane-agent skeleton; detected_os={os:?}");
}

fn detect_os() -> OperatingSystem {
    if cfg!(target_os = "linux") {
        OperatingSystem::Linux
    } else if cfg!(target_os = "freebsd") {
        OperatingSystem::FreeBsd
    } else if cfg!(target_os = "openbsd") {
        OperatingSystem::OpenBsd
    } else {
        OperatingSystem::Unknown
    }
}
