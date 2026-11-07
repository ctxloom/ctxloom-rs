//! The scaffold init asks for, wired: `config create` over the real files.

use init_application::Create;

use crate::commands::tests::World;
use crate::fixtures::exists;

use super::ConfigCreate;

fn scaffold(world: &World) -> ConfigCreate {
    let wiring = world.wiring();
    ConfigCreate(crate::library::build(&wiring, None).unwrap())
}

// A directory with no `.ctxloom` gets one: the scaffold is on disk afterwards.
#[test]
fn create_scaffolds_ctxloom_in_the_working_directory() {
    let world = World::new("setup-create");
    let app = world.dir().join("work").join(".ctxloom");
    assert!(!exists(&app));
    scaffold(&world).create().expect("scaffolded");
    assert!(exists(&app), "{} was not made", app.display());
}

// A scaffold that cannot be made is the library's own refusal, not Ok.
#[test]
fn create_fails_when_the_scaffold_cannot_be_made() {
    let world = World::new("setup-create-fails");
    let app = world.dir().join("work").join(".ctxloom");
    crate::fixtures::put_mode(&app, "in the way", 0o644);
    assert!(scaffold(&world).create().is_err());
}
