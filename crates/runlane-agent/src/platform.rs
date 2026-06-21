use runlane_core::{
    Capability, CapabilityFailure, CapabilityReport, EvidenceEnvelope, OperatingSystem,
    UnsupportedCapability,
};

pub trait PlatformBackend {
    fn os(&self) -> OperatingSystem;

    fn capability_report(&self, node_id: &str) -> CapabilityReport;

    fn parser_fixture_stubs(&self) -> &'static [&'static str];

    fn collector_specs(&self) -> &'static [CollectorSpec];

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

    fn collect_fixture(
        &self,
        kind: EvidenceKind,
        fixture_body: &str,
    ) -> Result<EvidenceEnvelope, CapabilityFailure> {
        let spec = self
            .collector_specs()
            .iter()
            .find(|spec| spec.kind == kind)
            .ok_or_else(|| {
                CapabilityFailure::Unsupported(UnsupportedCapability::new(
                    kind.capability_id(),
                    format!("{:?} backend has no collector for {kind:?}", self.os()),
                ))
            })?;
        self.require_capability(&Capability::new(spec.capability))?;
        if fixture_body.trim().is_empty() {
            return Err(CapabilityFailure::Unsupported(UnsupportedCapability::new(
                spec.capability,
                "fixture output was empty",
            )));
        }
        Ok(EvidenceEnvelope::text(
            spec.fixture,
            normalize_fixture(self.os(), kind, fixture_body),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceKind {
    ServiceStatus,
    RecentLogs,
    Disk,
    Process,
    Socket,
}

impl EvidenceKind {
    pub const fn capability_id(self) -> &'static str {
        match self {
            Self::ServiceStatus => "service.status",
            Self::RecentLogs => "logs.recent",
            Self::Disk => "storage.df",
            Self::Process => "process.snapshot",
            Self::Socket => "socket.snapshot",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectorSpec {
    pub kind: EvidenceKind,
    pub capability: &'static str,
    pub program: &'static str,
    pub args: &'static [&'static str],
    pub fixture: &'static str,
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
            "fixtures/linux/df.txt",
            "fixtures/linux/procfs-process.txt",
            "fixtures/linux/ss-listening.txt",
        ]
    }

    fn collector_specs(&self) -> &'static [CollectorSpec] {
        &[
            CollectorSpec {
                kind: EvidenceKind::ServiceStatus,
                capability: "service.systemd",
                program: "systemctl",
                args: &["show", "--no-pager", "sshd"],
                fixture: "fixtures/linux/systemctl-status.txt",
            },
            CollectorSpec {
                kind: EvidenceKind::RecentLogs,
                capability: "logs.journald",
                program: "journalctl",
                args: &["-u", "sshd", "--since", "-30m", "--no-pager"],
                fixture: "fixtures/linux/journalctl-service.txt",
            },
            CollectorSpec {
                kind: EvidenceKind::Disk,
                capability: "storage.df",
                program: "df",
                args: &["-P"],
                fixture: "fixtures/linux/df.txt",
            },
            CollectorSpec {
                kind: EvidenceKind::Process,
                capability: "process.procfs",
                program: "ps",
                args: &["-eo", "pid,stat,comm"],
                fixture: "fixtures/linux/procfs-process.txt",
            },
            CollectorSpec {
                kind: EvidenceKind::Socket,
                capability: "socket.ss",
                program: "ss",
                args: &["-ltnp"],
                fixture: "fixtures/linux/ss-listening.txt",
            },
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
            "fixtures/freebsd/messages.txt",
            "fixtures/freebsd/df.txt",
            "fixtures/freebsd/procstat.txt",
            "fixtures/freebsd/sockstat-listening.txt",
        ]
    }

    fn collector_specs(&self) -> &'static [CollectorSpec] {
        &[
            CollectorSpec {
                kind: EvidenceKind::ServiceStatus,
                capability: "service.freebsd-rc",
                program: "service",
                args: &["sshd", "status"],
                fixture: "fixtures/freebsd/service-status.txt",
            },
            CollectorSpec {
                kind: EvidenceKind::RecentLogs,
                capability: "logs.syslog-file",
                program: "tail",
                args: &["-n", "300", "/var/log/messages"],
                fixture: "fixtures/freebsd/messages.txt",
            },
            CollectorSpec {
                kind: EvidenceKind::Disk,
                capability: "storage.df",
                program: "df",
                args: &["-P"],
                fixture: "fixtures/freebsd/df.txt",
            },
            CollectorSpec {
                kind: EvidenceKind::Process,
                capability: "process.procstat",
                program: "procstat",
                args: &["-a"],
                fixture: "fixtures/freebsd/procstat.txt",
            },
            CollectorSpec {
                kind: EvidenceKind::Socket,
                capability: "socket.sockstat",
                program: "sockstat",
                args: &["-l4"],
                fixture: "fixtures/freebsd/sockstat-listening.txt",
            },
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
            "fixtures/openbsd/messages.txt",
            "fixtures/openbsd/df.txt",
            "fixtures/openbsd/ps.txt",
            "fixtures/openbsd/fstat-listening.txt",
        ]
    }

    fn collector_specs(&self) -> &'static [CollectorSpec] {
        &[
            CollectorSpec {
                kind: EvidenceKind::ServiceStatus,
                capability: "service.openbsd-rcctl",
                program: "rcctl",
                args: &["check", "sshd"],
                fixture: "fixtures/openbsd/rcctl-check.txt",
            },
            CollectorSpec {
                kind: EvidenceKind::RecentLogs,
                capability: "logs.syslog-file",
                program: "tail",
                args: &["-n", "300", "/var/log/messages"],
                fixture: "fixtures/openbsd/messages.txt",
            },
            CollectorSpec {
                kind: EvidenceKind::Disk,
                capability: "storage.df",
                program: "df",
                args: &["-P"],
                fixture: "fixtures/openbsd/df.txt",
            },
            CollectorSpec {
                kind: EvidenceKind::Process,
                capability: "process.ps",
                program: "ps",
                args: &["axo", "pid,stat,command"],
                fixture: "fixtures/openbsd/ps.txt",
            },
            CollectorSpec {
                kind: EvidenceKind::Socket,
                capability: "socket.fstat",
                program: "fstat",
                args: &["-n"],
                fixture: "fixtures/openbsd/fstat-listening.txt",
            },
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

    fn collector_specs(&self) -> &'static [CollectorSpec] {
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

    fn collector_specs(&self) -> &'static [CollectorSpec] {
        match self {
            Self::Linux(backend) => backend.collector_specs(),
            Self::FreeBsd(backend) => backend.collector_specs(),
            Self::OpenBsd(backend) => backend.collector_specs(),
            Self::Unknown(backend) => backend.collector_specs(),
        }
    }
}

fn normalize_fixture(os: OperatingSystem, kind: EvidenceKind, fixture_body: &str) -> String {
    let status = match kind {
        EvidenceKind::ServiceStatus
            if fixture_body.contains("ActiveState=active")
                || fixture_body.contains("is running")
                || fixture_body.contains("(ok)") =>
        {
            "service=active"
        }
        EvidenceKind::ServiceStatus => "service=not-active",
        EvidenceKind::RecentLogs => "logs=present",
        EvidenceKind::Disk => "disk=present",
        EvidenceKind::Process => "process=present",
        EvidenceKind::Socket => "socket=present",
    };
    format!(
        "os={os:?}; kind={kind:?}; {status}\n{}",
        fixture_body.trim()
    )
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
    use super::{
        EvidenceKind, FreeBsdBackend, LinuxBackend, OpenBsdBackend, PlatformBackend, UnknownBackend,
    };
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
        assert_eq!(LinuxBackend.parser_fixture_stubs().len(), 5);
        assert_eq!(FreeBsdBackend.parser_fixture_stubs().len(), 5);
        assert_eq!(OpenBsdBackend.parser_fixture_stubs().len(), 5);
    }

    #[test]
    fn collector_specs_use_native_commands() {
        let linux_service = LinuxBackend
            .collector_specs()
            .iter()
            .find(|spec| spec.kind == EvidenceKind::ServiceStatus)
            .expect("linux service collector exists");
        assert_eq!(linux_service.program, "systemctl");

        let freebsd_service = FreeBsdBackend
            .collector_specs()
            .iter()
            .find(|spec| spec.kind == EvidenceKind::ServiceStatus)
            .expect("freebsd service collector exists");
        assert_eq!(freebsd_service.program, "service");

        let openbsd_service = OpenBsdBackend
            .collector_specs()
            .iter()
            .find(|spec| spec.kind == EvidenceKind::ServiceStatus)
            .expect("openbsd service collector exists");
        assert_eq!(openbsd_service.program, "rcctl");
        assert_ne!(openbsd_service.program, "systemctl");
    }

    #[test]
    fn parses_service_unhealthy_fixtures_for_each_first_class_platform() {
        let cases: [(&dyn PlatformBackend, [(EvidenceKind, &str); 5]); 3] = [
            (
                &LinuxBackend,
                [
                    (
                        EvidenceKind::ServiceStatus,
                        include_str!("../fixtures/linux/systemctl-status.txt"),
                    ),
                    (
                        EvidenceKind::RecentLogs,
                        include_str!("../fixtures/linux/journalctl-service.txt"),
                    ),
                    (EvidenceKind::Disk, include_str!("../fixtures/linux/df.txt")),
                    (
                        EvidenceKind::Process,
                        include_str!("../fixtures/linux/procfs-process.txt"),
                    ),
                    (
                        EvidenceKind::Socket,
                        include_str!("../fixtures/linux/ss-listening.txt"),
                    ),
                ],
            ),
            (
                &FreeBsdBackend,
                [
                    (
                        EvidenceKind::ServiceStatus,
                        include_str!("../fixtures/freebsd/service-status.txt"),
                    ),
                    (
                        EvidenceKind::RecentLogs,
                        include_str!("../fixtures/freebsd/messages.txt"),
                    ),
                    (
                        EvidenceKind::Disk,
                        include_str!("../fixtures/freebsd/df.txt"),
                    ),
                    (
                        EvidenceKind::Process,
                        include_str!("../fixtures/freebsd/procstat.txt"),
                    ),
                    (
                        EvidenceKind::Socket,
                        include_str!("../fixtures/freebsd/sockstat-listening.txt"),
                    ),
                ],
            ),
            (
                &OpenBsdBackend,
                [
                    (
                        EvidenceKind::ServiceStatus,
                        include_str!("../fixtures/openbsd/rcctl-check.txt"),
                    ),
                    (
                        EvidenceKind::RecentLogs,
                        include_str!("../fixtures/openbsd/messages.txt"),
                    ),
                    (
                        EvidenceKind::Disk,
                        include_str!("../fixtures/openbsd/df.txt"),
                    ),
                    (
                        EvidenceKind::Process,
                        include_str!("../fixtures/openbsd/ps.txt"),
                    ),
                    (
                        EvidenceKind::Socket,
                        include_str!("../fixtures/openbsd/fstat-listening.txt"),
                    ),
                ],
            ),
        ];

        for (backend, fixtures) in cases {
            for (kind, body) in fixtures {
                let evidence = backend
                    .collect_fixture(kind, body)
                    .expect("fixture parses for backend");
                assert!(evidence.body.contains(&format!("os={:?}", backend.os())));
                assert!(evidence.body.contains(&format!("kind={kind:?}")));
            }
        }
    }

    #[test]
    fn unsupported_collectors_fail_closed() {
        let err = UnknownBackend
            .collect_fixture(EvidenceKind::ServiceStatus, "sshd(ok)")
            .expect_err("unknown backend has no collectors");
        assert!(matches!(err, CapabilityFailure::Unsupported(_)));
    }
}
