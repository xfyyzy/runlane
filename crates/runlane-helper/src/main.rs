use runlane_core::ActionKind;

fn main() {
    println!(
        "runlane-helper skeleton; privileged_action_surface={:?}",
        [ActionKind::ServiceRestart, ActionKind::ServiceReload]
    );
}
