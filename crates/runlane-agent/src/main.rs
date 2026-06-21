mod platform;

use platform::{NativeBackend, PlatformBackend};

fn main() {
    let backend = NativeBackend::current();
    let report = backend.capability_report("local-node");
    let fixture_stub_count = backend.parser_fixture_stubs().len();
    let capability_probe_ok = report
        .capabilities
        .first()
        .is_some_and(|capability| backend.require_capability(capability).is_ok());
    println!(
        "runlane-agent skeleton; detected_os={:?}; capabilities={}; unsupported={}; fixture_stubs={}; capability_probe_ok={}",
        report.os,
        report.capabilities.len(),
        report.unsupported.len(),
        fixture_stub_count,
        capability_probe_ok
    );
}
