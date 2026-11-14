//! Step definitions of `library_location_names_API-S-034.feature` (Task API-F2.31; API-S-034
//! **Folder and volume names**, API-S-038, DE-S-017): the folder-choice and facts routes of a
//! router over the library-routes rig, whose host dialog is scripted and whose desktop
//! registry reports the names a desktop would read. The desktop host's own reader over real
//! folders is covered by the desktop and native tests.

use crate::library_routes_fixture::{command, operation_id, Answer, LibraryRig, Mount};
use crate::MonitorWorld;
use cucumber::{given, then, when};
use filesquid_api::local_library::choice::{
    ChoicePurpose, ChooseError, ChosenFolder, LocationChooser,
};
use filesquid_api::local_library::FolderNames;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// A host dialog in which the owner picks the registered `location` whose names are `names`.
struct PickingDialog {
    location: String,
    names: FolderNames,
}

impl LocationChooser for PickingDialog {
    fn choose(&self, _purpose: ChoicePurpose) -> Result<Option<ChosenFolder>, ChooseError> {
        Ok(Some(ChosenFolder::new(
            self.location.clone(),
            self.names.clone(),
        )))
    }
}

/// Per-scenario state of the location-names steps.
#[derive(Default)]
pub struct LocationNamesWorld {
    /// The host.
    rig: Option<LibraryRig>,
    /// The location the dialog picks.
    location: Option<String>,
    /// The library created or opened there.
    library: Option<String>,
    /// The last answer.
    answer: Option<Answer>,
    /// Every answer and event text, and every log line, of the privacy scenario.
    seen: Vec<String>,
}

impl std::fmt::Debug for LocationNamesWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocationNamesWorld").finish_non_exhaustive()
    }
}

impl LocationNamesWorld {
    /// The host.
    fn rig(&self) -> &LibraryRig {
        self.rig.as_ref().expect("a host")
    }

    /// The last answer's JSON.
    fn json(&self) -> Value {
        self.answer.as_ref().expect("an answer").json()
    }

    /// A desktop host whose registered folder is answered with `names`, and whose dialog
    /// picks that folder.
    fn desktop_choosing(&mut self, names: FolderNames) {
        let rig = LibraryRig::new(Mount::Desktop);
        let location = rig.locations.register("chosen");
        rig.names
            .lock()
            .expect("names")
            .insert(location.clone(), names.clone());
        let dialog = PickingDialog {
            location: location.clone(),
            names,
        };
        assert!(rig.state.install_location_chooser(Arc::new(dialog)));
        self.rig = Some(rig);
        self.location = Some(location);
    }
}

/// Ask `rig` for a location for `purpose`.
async fn ask(rig: &LibraryRig, purpose: &str) -> Answer {
    let body = json!({ "purpose": purpose });
    rig.post("/api/v1/libraries/location-choices", body).await
}

/// Create a library at `location` of `rig`; answer its ID.
async fn create_at(rig: &LibraryRig, location: &str) -> String {
    let input = json!({ "location_id": location, "name": "Archive" });
    let body = command(&format!("location:{location}"), &operation_id(), "0", input);
    let answer = rig.post("/api/v1/libraries", body).await;
    assert_eq!(answer.status.as_u16(), 200, "{}", answer.text());
    answer.result()["library_id"]
        .as_str()
        .expect("library id")
        .to_string()
}

#[given(
    expr = "location-names desktop host whose dialog chooses the folder {string} on the volume {string}"
)]
fn desktop_dialog(w: &mut MonitorWorld, folder: String, volume: String) {
    let names = FolderNames::safe(Some(&folder), Some(&volume));
    w.location_names = LocationNamesWorld::default();
    w.location_names.desktop_choosing(names);
}

#[given(expr = "location-names desktop host that cannot read the volume name of the chosen folder")]
fn desktop_no_volume(w: &mut MonitorWorld) {
    w.location_names = LocationNamesWorld::default();
    w.location_names
        .desktop_choosing(FolderNames::safe(Some("Family archive"), None));
}

#[given(expr = "location-names library opened from the folder {string} on the volume {string}")]
async fn library_from(w: &mut MonitorWorld, folder: String, volume: String) {
    let names = FolderNames::safe(Some(&folder), Some(&volume));
    let world = &mut w.location_names;
    *world = LocationNamesWorld::default();
    world.desktop_choosing(names);
    let location = world.location.clone().expect("location");
    world.library = Some(create_at(world.rig(), &location).await);
}

#[given(expr = "location-names remembered library whose folder cannot be reached")]
async fn unreachable_library(w: &mut MonitorWorld) {
    let world = &mut w.location_names;
    *world = LocationNamesWorld::default();
    let rig = LibraryRig::new(Mount::Desktop);
    let location = rig.locations.register("gone");
    world.library = Some(create_at(&rig, &location).await);
    // The desktop reader answers no name for a folder it cannot reach.
    rig.names.lock().expect("names").remove(&location);
    world.rig = Some(rig);
}

#[given(expr = "location-names host without folder names")]
async fn host_without_names(w: &mut MonitorWorld) {
    let world = &mut w.location_names;
    *world = LocationNamesWorld::default();
    let rig = LibraryRig::new(Mount::Server);
    let location = rig.locations.register("served");
    let names = FolderNames::safe(Some("Family archive"), Some("Photos SSD"));
    rig.names
        .lock()
        .expect("names")
        .insert(location.clone(), names);
    world.library = Some(create_at(&rig, &location).await);
    world.rig = Some(rig);
}

#[when(expr = "location-names owner asks for a location for the purpose {word}")]
async fn owner_asks(w: &mut MonitorWorld, purpose: String) {
    let world = &mut w.location_names;
    world.answer = Some(ask(world.rig(), &purpose).await);
}

#[when(regex = r"^location-names (?:owner|caller) reads the library facts$")]
async fn reads_facts(w: &mut MonitorWorld) {
    let world = &mut w.location_names;
    let library = world.library.clone().expect("a library");
    let uri = format!("/api/v1/libraries/{library}/facts");
    world.answer = Some(world.rig().get(&uri).await);
}

/// Record every log line of this thread.
#[derive(Clone, Default)]
struct Lines(Arc<Mutex<Vec<String>>>);

/// Writes each field of an event into one line.
struct LineVisitor<'a>(&'a mut String);

impl tracing::field::Visit for LineVisitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.push_str(&format!("{}={value:?} ", field.name()));
    }
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Lines {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        let mut line = String::new();
        event.record(&mut LineVisitor(&mut line));
        self.0.lock().expect("lines").push(line);
    }
}

/// Ask for a location and create a library there on `rig`, on a current-thread runtime of
/// one blocking thread whose log lines are recorded; answer the rig, the library and every
/// text seen: both answers, every event and every log line.
fn choose_and_create_logged(rig: LibraryRig) -> (LibraryRig, String, Vec<String>) {
    use tracing_subscriber::layer::SubscriberExt;
    let lines = Lines::default();
    let subscriber = tracing_subscriber::registry().with(lines.clone());
    let _logs = tracing::subscriber::set_default(subscriber);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let mut events = rig.state.events.subscribe();
    let (answers, library) = runtime.block_on(async {
        let chosen = ask(&rig, "create").await;
        let location = chosen.json()["location_id"]
            .as_str()
            .expect("location")
            .to_string();
        let library = create_at(&rig, &location).await;
        (vec![chosen.text()], library)
    });
    tracing::info!(
        probe = "capture-live",
        "the recorder sees this thread's log lines"
    );
    let mut seen = answers;
    while let Ok(event) = events.try_recv() {
        seen.push(serde_json::to_string(&event).expect("event"));
    }
    let logged = lines.0.lock().expect("lines").clone();
    assert!(
        logged.iter().any(|l| l.contains("capture-live")),
        "capture live"
    );
    seen.push(format!("LOG {}", logged.join("\n")));
    (rig, library, seen)
}

#[when(expr = "location-names owner asks for a location and then creates a library there")]
async fn choose_and_create(w: &mut MonitorWorld) {
    let world = &mut w.location_names;
    let rig = world.rig.take().expect("a host");
    let run = tokio::task::spawn_blocking(move || choose_and_create_logged(rig));
    let (rig, library, seen) = run.await.expect("the logged run");
    world.rig = Some(rig);
    world.library = Some(library);
    world.seen = seen;
}

#[then(
    expr = "location-names answer should carry a location ID, folder_name {string} and volume_name {string}"
)]
fn answer_names(w: &mut MonitorWorld, folder: String, volume: String) {
    let json = w.location_names.json();
    assert_eq!(json["outcome"], "chosen", "{json}");
    assert!(json["location_id"]
        .as_str()
        .is_some_and(|l| l.starts_with("loc-")));
    assert_eq!(json["folder_name"], folder.as_str(), "{json}");
    assert_eq!(json["volume_name"], volume.as_str(), "{json}");
}

#[then(expr = "location-names answer should carry no path and no parent folder")]
fn no_path(w: &mut MonitorWorld) {
    let world = &w.location_names;
    let text = world.answer.as_ref().expect("an answer").text();
    let dir = world.rig().dir_text();
    assert!(!text.contains(&dir), "a path leaked: {text}");
    let parent = std::path::Path::new(&dir)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .expect("the rig folder's name");
    assert!(!text.contains(&parent), "a parent folder leaked: {text}");
    assert!(!text.contains('/') && !text.contains('\\'), "{text}");
}

#[then(expr = "location-names answer should carry folder_name and a null volume_name")]
fn answer_null_volume(w: &mut MonitorWorld) {
    let json = w.location_names.json();
    assert_eq!(json["folder_name"], "Family archive", "{json}");
    assert_eq!(json["volume_name"], Value::Null, "{json}");
}

#[then(expr = "location-names facts should carry folder_name {string} and volume_name {string}")]
fn facts_names(w: &mut MonitorWorld, folder: String, volume: String) {
    let json = w.location_names.json();
    assert_eq!(json["folder_name"], folder.as_str(), "{json}");
    assert_eq!(json["volume_name"], volume.as_str(), "{json}");
}

#[then(expr = "location-names facts should carry a null folder_name and a null volume_name")]
fn facts_null(w: &mut MonitorWorld) {
    let world = &w.location_names;
    let answer = world.answer.as_ref().expect("an answer");
    assert_eq!(answer.status.as_u16(), 200, "{}", answer.text());
    let json = answer.json();
    assert_eq!(json["folder_name"], Value::Null, "{json}");
    assert_eq!(json["volume_name"], Value::Null, "{json}");
}

#[then(
    expr = "location-names journal, events and log lines should contain neither {string} nor {string}"
)]
fn nothing_names(w: &mut MonitorWorld, folder: String, volume: String) {
    let world = &w.location_names;
    let journal = serde_json::to_string(&world.rig().journal()).expect("journal");
    let (answers, rest) = world.seen.split_at(1);
    assert!(answers[0].contains(&folder), "the answer names the folder");
    for text in rest.iter().chain(std::iter::once(&journal)) {
        assert!(!text.contains(&folder), "{folder} leaked: {text}");
        assert!(!text.contains(&volume), "{volume} leaked: {text}");
    }
}

#[then(expr = "location-names journal should name only the opaque location and library IDs")]
fn journal_opaque(w: &mut MonitorWorld) {
    let world = &w.location_names;
    let location = world.location.clone().expect("location");
    let library = world.library.clone().expect("library");
    let choices = world.rig().journaled("library.location.choose");
    assert_eq!(choices.len(), 1, "journaled once");
    assert_eq!(choices[0].target_ids, std::slice::from_ref(&location));
    let creates = world
        .rig()
        .journaled(filesquid_api::local_library::CREATE_OPERATION);
    assert_eq!(creates.len(), 1, "the create journaled once");
    assert!(creates[0].target_ids.contains(&library), "{:?}", creates[0]);
    for target in &creates[0].target_ids {
        assert!(
            [location.as_str(), library.as_str()].contains(&target.as_str()),
            "only opaque IDs: {target}"
        );
    }
}
