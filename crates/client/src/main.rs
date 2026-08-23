// Desktop binary entry point — everything actually lives in lib.rs (see its
// own #[bevy_main] doc comment) so the exact same App-building code also
// works as the staticlib iOS/Android builds link against.
fn main() {
    client::main();
}
