use runlane_core::{
    Capability, CapabilityFailure, CapabilityReport, OperatingSystem, UnsupportedCapability,
};

pub trait PlatformBackend {
    fn os(&self) -> OperatingSystem;

    fn capability_report(&self, node_id: &str) -> CapabilityReport;

    fn parser_fixture_stubs(&self) -> &'static [&'static str];

    fn require_capability(&self, capability: &Capability) -> Result<(), CapabilityFailure> {
        let report = self.capability_report("capability-check");
        if report.supports(capability) {
            Ok(())
        } else {
            let unsupported = report
                .unsupported
                .into_iter()
                .find(|entry| entry.capability == *capability)
                .unwrap_or_else(|| {
                    UnsupportedCapability::new(
                        capability.as_str(),
                        format!("{:?} backend does not report this capability", self.os()),
                    )
                });
            Err(CapabilityFailure::Unsupported(unsupported))
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LinuxBackend;

#[derive(Debug, Clone, Copy)]
pub struct FreeBsdBackend;

#[derive(Debug, Clone, Copy)]
pub struct OpenBsdBackend;

#[derive(Debug, Clone, Copy)]
pub struct UnknownBackend;

impl PlatformBackend for LinuxBackend {
    fn os(&self) -> OperatingSystem {
        OperatingSystem::Linux
    }

    fn capability_report(&self, node_id: &str) -> CapabilityReport {
        CapabilityReport::new(
            node_id,
            self.os(),
            capabilities([
                "os.linux",
                "service.systemd",
                "logs.journald",
                "logs.syslog-file",
                "process.procfs",
                "process.ps",
                "socket.ss",
                "storage.df",
                "privilege.sudo-helper",
            ]),
            unsupported([
                (
                    "service.freebsd-rc",
                    "Linux backend reports systemd for v0.1 service management",
                ),
                (
                    "service.openbsd-rcctl",
                    "Linux backend reports systemd for v0.1 service management",
                ),
            ]),
        )
    }

    fn parser_fixture_stubs(&self) -> &'static [&'static str] {
        &[
            "fixtures/linux/systemctl-status.txt",
            "fixtures/linux/journalctl-service.txt",
            "fixtures/linux/ss-listening.txt",
        ]
    }
}

impl PlatformBackend for FreeBsdBackend {
    fn os(&self) -> OperatingSystem {
        OperatingSystem::FreeBsd
    }

    fn capability_report(&self, node_id: &str) -> CapabilityReport {
        CapabilityReport::new(
            node_id,
            self.os(),
            capabilities([
                "os.freebsd",
                "service.freebsd-rc",
                "logs.syslog-file",
                "process.procstat",
                "process.ps",
                "socket.sockstat",
                "storage.df",
                "package.freebsd-pkg",
                "privilege.sudo-helper",
            ]),
            unsupported([
                ("service.systemd", "FreeBSD uses rc.d/service, not systemd"),
                ("logs.journald", "FreeBSD v0.1 backend uses syslog files"),
                (
                    "service.openbsd-rcctl",
                    "FreeBSD uses rc.d/service, not rcctl",
                ),
            ]),
        )
    }

    fn parser_fixture_stubs(&self) -> &'static [&'static str] {
        &[
            "fixtures/freebsd/service-status.txt",
            "fixtures/freebsd/sockstat-listening.txt",
            "fixtures/freebsd/procstat.txt",
        ]
    }
}

impl PlatformBackend for OpenBsdBackend {
    fn os(&self) -> OperatingSystem {
        OperatingSystem::OpenBsd
    }

    fn capability_report(&self, node_id: &str) -> CapabilityReport {
        CapabilityReport::new(
            node_id,
            self.os(),
            capabilities([
                "os.openbsd",
                "service.openbsd-rcctl",
                "logs.syslog-file",
                "process.ps",
                "socket.fstat",
                "storage.df",
                "package.openbsd-pkg-info",
                "privilege.doas-helper",
            ]),
            unsupported([
                ("service.systemd", "OpenBSD uses rcctl, not systemd"),
                ("logs.journald", "OpenBSD v0.1 backend uses syslog files"),
                (
                    "service.freebsd-rc",
                    "OpenBSD uses rcctl, not FreeBSD rc.d/service",
                ),
            ]),
        )
    }

    fn parser_fixture_stubs(&self) -> &'static [&'static str] {
        &[
            "fixtures/openbsd/rcctl-check.txt",
            "fixtures/openbsd/fstat-listening.txt",
            "fixtures/openbsd/ps.txt",
        ]
    }
}

impl PlatformBackend for UnknownBackend {
    fn os(&self) -> OperatingSystem {
        OperatingSystem::Unknown
    }

    fn capability_report(&self, node_id: &str) -> CapabilityReport {
        CapabilityReport::new(
            node_id,
            self.os(),
            [],
            [UnsupportedCapability::new(
                "platform.native-backend",
                "no native backend is available for this target OS",
            )],
        )
    }

    fn parser_fixture_stubs(&self) -> &'static [&'static str] {
        &[]
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NativeBackend {
    Linux(LinuxBackend),
    FreeBsd(FreeBsdBackend),
    OpenBsd(OpenBsdBackend),
    Unknown(UnknownBackend),
}

impl NativeBackend {
    pub fn current() -> Self {
        if cfg!(target_os = "linux") {
            Self::Linux(LinuxBackend)
        } else if cfg!(target_os = "freebsd") {
            Self::FreeBsd(FreeBsdBackend)
        } else if cfg!(target_os = "openbsd") {
            Self::OpenBsd(OpenBsdBackend)
        } else {
            Self::Unknown(UnknownBackend)
        }
    }
}

impl PlatformBackend for NativeBackend {
    fn os(&self) -> OperatingSystem {
        match self {
            Self::Linux(backend) => backend.os(),
            Self::FreeBsd(backend) => backend.os(),
            Self::OpenBsd(backend) => backend.os(),
            Self::Unknown(backend) => backend.os(),
        }
    }

    fn capability_report(&self, node_id: &str) -> CapabilityReport {
        match self {
            Self::Linux(backend) => backend.capability_report(node_id),
            Self::FreeBsd(backend) => backend.capability_report(node_id),
            Self::OpenBsd(backend) => backend.capability_report(node_id),
            Self::Unknown(backend) => backend.capability_report(node_id),
        }
    }

    fn parser_fixture_stubs(&self) -> &'static [&'static str] {
        match self {
            Self::Linux(backend) => backend.parser_fixture_stubs(),
            Self::FreeBsd(backend) => backend.parser_fixture_stubs(),
            Self::OpenBsd(backend) => backend.parser_fixture_stubs(),
            Self::Unknown(backend) => backend.parser_fixture_stubs(),
        }
    }
}

fn capabilities<const N: usize>(values: [&str; N]) -> Vec<Capability> {
    values.into_iter().map(Capability::new).collect()
}

fn unsupported<const N: usize>(values: [(&str, &str); N]) -> Vec<UnsupportedCapability> {
    values
        .into_iter()
        .map(|(capability, reason)| UnsupportedCapability::new(capability, reason))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{FreeBsdBackend, LinuxBackend, OpenBsdBackend, PlatformBackend};
    use runlane_core::{Capability, CapabilityFailure, OperatingSystem};

    #[test]
    fn reports_distinct_native_capabilities_for_tier_one_platforms() {
        let linux = LinuxBackend.capability_report("linux-01");
        let freebsd = FreeBsdBackend.capability_report("freebsd-01");
        let openbsd = OpenBsdBackend.capability_report("openbsd-01");

        assert_eq!(linux.os, OperatingSystem::Linux);
        assert!(linux.supports(&Capability::new("service.systemd")));
        assert!(!linux.supports(&Capability::new("service.openbsd-rcctl")));

        assert_eq!(freebsd.os, OperatingSystem::FreeBsd);
        assert!(freebsd.supports(&Capability::new("service.freebsd-rc")));
        assert!(!freebsd.supports(&Capability::new("service.systemd")));

        assert_eq!(openbsd.os, OperatingSystem::OpenBsd);
        assert!(openbsd.supports(&Capability::new("service.openbsd-rcctl")));
        assert!(!openbsd.supports(&Capability::new("service.systemd")));
    }

    #[test]
    fn unsupported_capabilities_fail_closed_with_reason() {
        let err = OpenBsdBackend
            .require_capability(&Capability::new("service.systemd"))
            .expect_err("OpenBSD backend must not silently emulate systemd");

        match err {
            CapabilityFailure::Unsupported(unsupported) => {
                assert_eq!(unsupported.capability.as_str(), "service.systemd");
                assert!(unsupported.reason.contains("rcctl"));
            }
            CapabilityFailure::BackendUnavailable { .. } => {
                panic!("expected unsupported capability")
            }
        }
    }

    #[test]
    fn parser_fixture_stubs_cover_each_os_family() {
        assert!(
            LinuxBackend
                .parser_fixture_stubs()
                .iter()
                .any(|name| name.contains("linux"))
        );
        assert!(
            FreeBsdBackend
                .parser_fixture_stubs()
                .iter()
                .any(|name| name.contains("freebsd"))
        );
        assert!(
            OpenBsdBackend
                .parser_fixture_stubs()
                .iter()
                .any(|name| name.contains("openbsd"))
        );
    }
}
