# Task DE-F3.1 Walkthrough: macOS early-access DMG and the FileSquid overlay

Branch `feat/de-f3.1-macos-dmg-overlay`. Module row [DE-F3.1](../plan.md) of global [Task FS-3.1](../../../../../../resources/implementation-plan.md#task-fs-3.1).

**The row is open.** Owner decision 2026-11-11, verbatim: "yes run DE-F3.1, I'll do my own manual mac and windows testing". This work was done on Linux. Everything below up to the heading "Owner's macOS test steps" was run on Linux. Nothing under that heading has been run. The DE-F3.1 box and the Task FS-3.1 box stay unticked until the owner's macOS run passes.

## What changed

- [`tauri.filesquid.json`](../../tauri.filesquid.json) (new): the `filesquid` overlay. It sets `productName` "FileSquid", `identifier` `tech.filesquid.desktop`, the window title, `build.frontendDist` `../../ui/dist` with its build and dev commands, and `bundle.icon`.
- [`tauri.bundle.macos-dmg.json`](../../tauri.bundle.macos-dmg.json) (new): the bundling overlay of the macOS disk image. `bundle.active` true, `bundle.targets` `["dmg"]`, `bundle.macOS.signingIdentity` `-` (ad hoc), `bundle.macOS.hardenedRuntime` false. It carries no brand, so another distribution passes its own overlay first and this one second.
- [`tauri.conf.json`](../../tauri.conf.json) is unchanged: `bundle.active` stays false, so no build makes an installer unless the bundling overlay is named. Neither overlay is a `tauri.<platform>.conf.json`, so the CLI merges nothing on its own.
- [`icons/`](../../icons/): `app-icon.svg` (new) is the FileSquid mark of the UI package on a white rounded square. `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png` and `icon.icns` were generated from it with `npx tauri icon`. They replace three placeholder squares.
- [`scripts/linked-inventory.sh`](../../scripts/linked-inventory.sh) (new): the linked inventory for the ADR 011 review.
- Tests: [`tests/integration/distribution_overlay.rs`](../../tests/integration/distribution_overlay.rs), [`tests/integration/linked_inventory.rs`](../../tests/integration/linked_inventory.rs), [`tests/features/desktop_distribution_overlay_F-058.feature`](../../tests/features/desktop_distribution_overlay_F-058.feature) with [`tests/support/overlay_steps.rs`](../../tests/support/overlay_steps.rs) and [`tests/support/overlay.rs`](../../tests/support/overlay.rs), and the packaged-window spec [`tests/e2e/specs/branding.e2e.mjs`](../../tests/e2e/specs/branding.e2e.mjs).
- `scripts/verify_tests.sh`: the opt-in desktop packaged-window step (`FILESQUID_DESKTOP_E2E=1`) now builds the production UI before the test-instrumented binary, because the binary embeds the UI at compile time and the branding spec needs the real UI.
- Root `README.md`: the FileSquid overlay command, the two overlays, the inventory script, the new test command and the UI build before the packaged-window tests.
- [`plan.md`](../plan.md): a Status line on DE-F3.1 with the owner's decision. No box ticked.

No file under `src/` changed. No file in `apps/filesquid/ui` changed: the About view already shows the statement (`settingsBuildNote` in the label table, rendered by `fs-settings.ts`).

## DE-S-007: One desktop binary MUST serve every distribution through a Tauri config overlay (product name, identifier, icons, UI directory)

| Constraint | Proof on Linux |
|---|---|
| The `filesquid` overlay names the product, the identifier, the icons and the UI directory | `the_filesquid_overlay_names_the_product_and_the_identifier`, `…_names_the_default_ui_directory`, `…_lists_icons_that_exist_with_a_macos_icon_among_them`, `the_macos_icon_is_an_apple_icon_image`; feature scenario "The FileSquid overlay brands the desktop build" |
| The overlay flag is `--config`, it may be repeated, and later values win | The pinned CLI's own help (below) and two real builds through it; `later_overlays_win_in_the_order_the_cli_merges_them` |
| Another distribution reuses the bundling overlay unchanged | `the_dmg_overlay_carries_no_brand_so_every_distribution_reuses_it`; scenario "Another distribution reuses the installer overlay unchanged" |
| The base configuration alone builds no installer | `the_base_configuration_alone_builds_no_installer`, `no_platform_file_merges_bundling_in_without_being_named`, `the_filesquid_overlay_turns_no_bundling_on` |
| The window's permissions do not change under the overlays (DE-S-001, DE-S-002) | `the_capability_policy_holds_with_the_overlays_merged` for macOS, Linux and Windows; the scenario outline and the refused widening overlay |

## ADR 035 Decision 3: Unsigned early-access DMG

| Constraint | Proof on Linux | Left for macOS |
|---|---|---|
| A `.dmg` built by the Tauri bundler | `the_dmg_overlay_builds_a_dmg_and_no_other_installer`; the CLI accepts the overlay | Building it (step 1) |
| Not signed with a Developer ID, not notarized | `the_dmg_overlay_signs_ad_hoc_and_never_with_a_developer_id` (no `providerShortName`) | `codesign` and `spctl` output (step 2) |
| An ad-hoc signature so it launches on Apple silicon | `signingIdentity` is `-` in the overlay; the CLI accepts it and refuses a non-string | `Signature=adhoc` (step 2), a launch on Apple silicon (step 3) |
| No updater (Task 8.5) | `the_dmg_overlay_configures_no_updater` | |
| The app shows no early-access label | Packaged-window spec, test 4 | Same spec on macOS (step 4), and by eye in the installed app (step 3) |
| About states "This macOS build is not signed or notarised by Apple yet." | Packaged-window spec, test 3 | Same |

`signingIdentity: "-"` is the ad-hoc pseudo-identity of the Tauri macOS signing guide (`https://v2.tauri.app/distribute/sign/macos/`). The Linux build of the CLI does not contain the macOS signing code, so only its configuration check could be run here.

`hardenedRuntime` is false by my choice, and it needs the owner's check. The media workers load libvips at run time from outside the app (below). With the hardened runtime on, macOS library validation refuses a library that the app's own signer did not sign, and an ad-hoc signature has no signer identity. Notarization needs the hardened runtime, so Task 8.5 has to revisit this.

## UI-S-033: Settings page composition (the About box) and UI-S-025: Shell composition (the brand)

[`branding.e2e.mjs`](../../tests/e2e/specs/branding.e2e.mjs) reads the real window through every open shadow root:

1. The window runs the production UI and not the build script's stub page.
2. The sidebar shows the product name "FileSquid" and a logo image that decoded, whose file name contains `filesquid-mark`, with a text alternative. The page title is "FileSquid".
3. On `#/settings`, the About box shows a decoded logo, "FileSquid 0.1.0" and the exact statement.
4. No text, `alt`, `title`, `aria-label` or `placeholder` on the Settings page or the file browser matches `early access` in any spelling or case.

The native window title and the Dock name come from the Tauri configuration, which a WebDriver session cannot read. The overlay tests cover the configuration; the owner's step 3 covers the installed app.

## Evidence (run on Linux, Ubuntu 24.04, x86_64)

### The pinned Tauri CLI

`@tauri-apps/cli` is pinned `~2.12` in `apps/filesquid/ui/package.json`. `npx tauri --version` there prints `tauri-cli 2.12.3`. From `npx tauri build --help`:

```text
  -b, --bundles [<BUNDLES>...]
          Space or comma separated list of bundles to package
          
          [possible values: deb, rpm, appimage]

      --no-bundle
          Skip the bundling step even if `bundle > active` is `true` in tauri config

  -c, --config <CONFIG>
          JSON strings or paths to JSON, JSON5 or TOML files to merge with the default configuration file
          
          Configurations are merged in the order they are provided, which means a particular value overwrites previous values when a config key-value pair conflicts.
          
          Note that a platform-specific file is looked up and merged with the default file by default (tauri.macos.conf.json, tauri.linux.conf.json, tauri.windows.conf.json, tauri.android.conf.json and tauri.ios.conf.json) but you can use this for more specific use cases such as different build flavors.
```

On Linux the CLI offers no `dmg` bundle, so the disk image itself cannot be built here.

The FileSquid overlay through the CLI (`cd apps/filesquid/ui && npm run desktop:build -- --debug --no-bundle --config tauri.filesquid.json`), last lines:

```text
    Finished `dev` profile [unoptimized] target(s) in 46.49s
       Built application at: <target>/debug/filesquid-desktop
```

Both overlays, with the test feature, for the packaged-window run (`npm run desktop:build -- --debug --no-bundle --config tauri.filesquid.json --config tauri.bundle.macos-dmg.json --features test-webdriver`):

```text
> @filesquid/ui@0.1.0 desktop:build
> cd ../crates/desktop && tauri build --debug --no-bundle --config tauri.filesquid.json --config tauri.bundle.macos-dmg.json --features test-webdriver
    Finished `dev` profile [unoptimized] target(s) in 8.43s
       Built application at: <target>/debug/filesquid-desktop
```

The CLI checks an overlay against its configuration schema. With `--config '{"bundle":{"macOS":{"signingIdentity":5}}}'` in place of the bundling overlay it stops with:

```text
       Error `"tauri.conf.json"` error on `bundle > macOS > signingIdentity`: 5 is not of types "null", "string"
```

### Tests failed first

Before the overlay files and icons existed (`cargo test -p filesquid-desktop --locked --test integration_distribution_overlay`):

```text
test no_platform_file_merges_bundling_in_without_being_named ... ok
test later_overlays_win_in_the_order_the_cli_merges_them ... FAILED
test the_capability_policy_holds_with_the_overlays_merged ... FAILED
test the_base_configuration_alone_builds_no_installer ... FAILED
test the_dmg_overlay_builds_a_dmg_and_no_other_installer ... FAILED
test the_dmg_overlay_carries_no_brand_so_every_distribution_reuses_it ... FAILED
test the_dmg_overlay_configures_no_updater ... FAILED
test the_dmg_overlay_signs_ad_hoc_and_never_with_a_developer_id ... FAILED
test the_dmg_overlay_leaves_the_hardened_runtime_off_for_the_run_time_loaded_libvips ... FAILED
test the_filesquid_overlay_lists_icons_that_exist_with_a_macos_icon_among_them ... FAILED
test the_filesquid_overlay_names_the_default_ui_directory ... FAILED
test the_filesquid_overlay_names_the_product_and_the_identifier ... FAILED
test the_filesquid_overlay_changes_nothing_the_base_configuration_decides_about_security ... FAILED
test the_filesquid_overlay_turns_no_bundling_on ... FAILED
test the_macos_icon_is_an_apple_icon_image ... FAILED
test result: FAILED. 1 passed; 14 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

The inventory script and its tests were written together, not test-first. Its tests then caught two defects, which were fixed: `--without-vips` still looked for libvips, and the ImageMagick check matched a folder name. A later reading of the script found that a symbolic-link loop was followed without a bound and that the macOS parsing split paths at spaces; both were fixed, the first with a test. The first AI PR Agent review found that a bundled ImageMagick inside the subject escaped the check, and that an unresolved library or a failed `cargo tree` still exited 0; three tests were written first and failed, then both were fixed. The second review found that a `--runtime-lib` that is not a readable file was accepted, that the Linux parser cut a library path at its first space, and that in a mounted volume a bundled library could be taken for the main executable; five tests were written first and failed, then all three were fixed (a missing run-time library and a file that is not an object file are now findings with exit status 1, and the script prints the main executable it chose). The third review found that on macOS a dependency named by a path that is not a file was listed without a finding, that a failing `ldd` or `otool` was taken as "no libraries", and that a run path inherited from the main executable was expanded relative to the library that used it. The first two were fixed with three tests on Linux (a stand-in `ldd` that names a missing file, one that fails, and one that reports a statically linked file, which is not a failure); the third is a macOS-only change with no test on this machine. The fourth review found that `otool -L` also prints a dylib's own install name, which was taken for a dependency, and that run paths inherited along the loading chain were not searched. Both are macOS-only changes with no test on this machine: the script leaves out the name `otool -D` reports, records which image loaded each library, and searches the run paths of the loader, then of each image up the chain, then of the main executable, each expanded relative to the image that declares it.

The merge of main at fd8d8b9 (API-F2.30) made two existing event-channel scenarios of the desktop BDD harness fail in some runs: its `drain` helper stopped the pump after a fixed 50 ms, which raced the forwarder on a loaded machine. `drain` now stops once an event has arrived and 50 ms pass without another, and after 5 s at the latest; the features then passed 6 runs of 6 (79 scenarios). On origin/main alone (71 scenarios) the same test had passed 4 runs of 4.

The branding spec against a binary built over the stub page (the UI not built):

```text
[WebKitGTK 605.1.15 linux #0-1]    ✖ runs the production UI, not the build script's stub page
[WebKitGTK 605.1.15 linux #0-1]    ✖ shows the product name and the logo mark at the top of the sidebar
[WebKitGTK 605.1.15 linux #0-1]    ✖ states in Settings, About that the macOS build is not signed or notarised
[WebKitGTK 605.1.15 linux #0-1]    ✖ shows no early-access label on the Settings page or the file browser
[WebKitGTK 605.1.15 linux #0-1] the window shows no <fs-app-shell>: build apps/filesquid/ui (npm run build) and then rebuild the test-instrumented binary, so it embeds the production UI
```

With `FILESQUID_E2E_PRODUCT=ImageSquid FILESQUID_E2E_LOGO=imagesquid` against the FileSquid build, tests 2 and 3 fail (`Expected: "ImageSquid"`, `Received: "FileSquid"`), so the spec does read the brand.

### Tests passing

`cargo test -p filesquid-desktop --locked --test integration_distribution_overlay --test integration_linked_inventory`:

```text
     Running tests/integration/distribution_overlay.rs (integration_distribution_overlay)
test no_platform_file_merges_bundling_in_without_being_named ... ok
test the_base_configuration_alone_builds_no_installer ... ok
test later_overlays_win_in_the_order_the_cli_merges_them ... ok
test the_dmg_overlay_builds_a_dmg_and_no_other_installer ... ok
test the_dmg_overlay_carries_no_brand_so_every_distribution_reuses_it ... ok
test the_dmg_overlay_leaves_the_hardened_runtime_off_for_the_run_time_loaded_libvips ... ok
test the_dmg_overlay_signs_ad_hoc_and_never_with_a_developer_id ... ok
test the_dmg_overlay_configures_no_updater ... ok
test the_filesquid_overlay_changes_nothing_the_base_configuration_decides_about_security ... ok
test the_filesquid_overlay_names_the_default_ui_directory ... ok
test the_filesquid_overlay_lists_icons_that_exist_with_a_macos_icon_among_them ... ok
test the_filesquid_overlay_names_the_product_and_the_identifier ... ok
test the_filesquid_overlay_turns_no_bundling_on ... ok
test the_macos_icon_is_an_apple_icon_image ... ok
test the_capability_policy_holds_with_the_overlays_merged ... ok
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/integration/linked_inventory.rs (integration_linked_inventory)
test it_accepts_a_statically_linked_file_as_having_no_libraries ... ok
test it_fails_when_imagemagick_is_bundled_inside_the_subject ... ok
test it_fails_when_the_rust_crates_cannot_be_listed ... ok
test it_fails_when_imagemagick_is_in_the_inventory ... ok
test it_fails_when_a_linked_library_cannot_be_found ... ok
test it_finds_the_app_executable_inside_a_mounted_volume ... ok
test it_lists_the_dynamic_libraries_of_a_real_binary ... ok
test it_refuses_a_folder_with_no_executable_in_it ... ok
test it_refuses_a_subject_that_does_not_exist ... ok
test it_keeps_the_whole_path_of_a_library_in_a_folder_with_spaces ... ok
test it_reports_a_library_whose_inspection_fails ... ok
test it_reports_a_resolved_library_path_that_is_not_a_file ... ok
test it_lists_the_run_time_loaded_libvips_and_its_loader_modules ... ok
test it_reports_a_library_it_cannot_inspect ... ok
test it_lists_the_rust_crates_with_their_licences ... ok
test it_reports_a_run_time_library_that_does_not_exist ... ok
test it_reports_a_run_time_library_reached_through_a_symbolic_link_loop_as_missing ... ok
test it_says_when_libvips_is_missing_and_lists_it_when_it_is_installed ... ok
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 23.39s
```

`cargo test -p filesquid-desktop --locked --features test-webdriver --test features`, the new feature and the summary:

```text
Feature: One desktop binary is branded and packaged through configuration overlays
  Scenario: The FileSquid overlay brands the desktop build
   ✔  Given the desktop build configuration with the "filesquid" overlay
   ✔  Then the product name should be "FileSquid"
   ✔  And the application identifier should be "tech.filesquid.desktop"
   ✔  And the main window title should be "FileSquid"
   ✔  And the UI directory should be "../../ui/dist"
   ✔  And every listed icon should exist, with a macOS icon among them
  Scenario: The macOS installer overlay asks for an early-access disk image
   ✔  Given the desktop build configuration with the "filesquid" and "macos-dmg" overlays
   ✔  Then the build should produce installers
   ✔  And the only installer should be "dmg"
   ✔  And the app should be signed ad hoc, without a Developer ID
   ✔  And no updater should be configured
  Scenario: Without the installer overlay no installer is built
   ✔  Given the desktop build configuration with the "filesquid" overlay
   ✔  Then the build should produce no installers
  Scenario: Another distribution reuses the installer overlay unchanged
   ✔  Given the desktop build configuration with another distribution's overlay and the "macos-dmg" overlay
   ✔  Then the product name should be "ImageSquid"
   ✔  And the only installer should be "dmg"
   ✔  And the app should be signed ad hoc, without a Developer ID
  Scenario Outline: The window's permissions stay the same under the overlays on macos
   ✔  Given the desktop build configuration with the "filesquid" and "macos-dmg" overlays
   ✔  When the capability policy checks that configuration for "macos"
   ✔  Then the configuration should pass the capability policy
  Scenario Outline: The window's permissions stay the same under the overlays on linux
   ✔  Given the desktop build configuration with the "filesquid" and "macos-dmg" overlays
   ✔  When the capability policy checks that configuration for "linux"
   ✔  Then the configuration should pass the capability policy
  Scenario Outline: The window's permissions stay the same under the overlays on windows
   ✔  Given the desktop build configuration with the "filesquid" and "macos-dmg" overlays
   ✔  When the capability policy checks that configuration for "windows"
   ✔  Then the configuration should pass the capability policy
  Scenario: An overlay that gives the window more permissions is refused
   ✔  Given the desktop build configuration with the "filesquid" overlay and one that enables the capability "wide"
   ✔  When the capability policy checks that configuration for "macos"
   ✔  Then the policy should refuse the enabled capability list "default, wide"
[Summary]
8 features
72 scenarios (72 passed)
326 steps (326 passed)
```

The packaged-window branding spec, against the binary the CLI built with both overlays, under Xvfb (`xvfb-run -a npm run test:e2e` in `tests/e2e`):

```text
[WebKitGTK 605.1.15 linux #0-0] Running: WebKitGTK (v605.1.15) on linux
[WebKitGTK 605.1.15 linux #0-0] » specs/branding.e2e.mjs
[WebKitGTK 605.1.15 linux #0-0] the distribution's brand in a packaged window
[WebKitGTK 605.1.15 linux #0-0]    ✓ runs the production UI, not the build script's stub page
[WebKitGTK 605.1.15 linux #0-0]    ✓ shows the product name and the logo mark at the top of the sidebar
[WebKitGTK 605.1.15 linux #0-0]    ✓ states in Settings, About that the macOS build is not signed or notarised
[WebKitGTK 605.1.15 linux #0-0]    ✓ shows no early-access label on the Settings page or the file browser
[WebKitGTK 605.1.15 linux #0-0] 4 passing (142ms)
Spec Files:	 7 passed, 1 failed, 8 total (100% completed) in 00:00:20  
```

That run's one failed spec file is `libraries.e2e.mjs`, with two failing tests ("opens, copies and rebuilds the media library through its location ID": the rebuild job lists 2 objects where the opening counted 1; and the repair test after it, 409 for 200). It is not caused by this branch: the same two tests fail when `libraries.e2e.mjs` is run alone, and when the binary is built with plain `cargo build` over the stub page, and this branch changes no file under `src/`. The other seven spec files pass. The hook runs this suite only with `FILESQUID_DESKTOP_E2E=1`, so a `FULL=1` run does not show it.

### Linked inventory of the Linux build

`bash crates/desktop/scripts/linked-inventory.sh "${CARGO_TARGET_DIR:-target}/debug/filesquid-desktop"` from `apps/filesquid`, exit status 2. This proves the method. It is not the DMG's inventory. Counts: 128 `lib link` rows, 60 `lib runtime` rows, 311 `crate` rows. All rows except `lib` and `crate`:

```text
# FileSquid linked inventory (ADR 011 review input)
# subject: filesquid-desktop
# platform: Linux x86_64
# main executable: filesquid-desktop
## Files scanned
file	<target>/debug/filesquid-desktop
## Run-time loaded libvips
vips	library	/usr/lib/x86_64-linux-gnu/libvips.so.42.17.1
vips	build-options	unknown: no vips program found to report them
module	/usr/lib/x86_64-linux-gnu/vips-modules-8.15/vips-heif.so
module	/usr/lib/x86_64-linux-gnu/vips-modules-8.15/vips-jxl.so
module	/usr/lib/x86_64-linux-gnu/vips-modules-8.15/vips-magick.so
module	/usr/lib/x86_64-linux-gnu/vips-modules-8.15/vips-openslide.so
module	/usr/lib/x86_64-linux-gnu/vips-modules-8.15/vips-poppler.so
## Dynamic libraries
## Rust crates linked into the binary
## Findings
finding	imagemagick	/usr/lib/x86_64-linux-gnu/libMagickCore-6.Q16.so.7.0.0
finding	imagemagick	/usr/lib/x86_64-linux-gnu/vips-modules-8.15/vips-magick.so
```

A sample of the `lib` rows (origin, owner, path):

```text
lib	link	dpkg:libc6:amd64 2.39-0ubuntu8.9	/usr/lib/x86_64-linux-gnu/libc.so.6
lib	link	dpkg:libgtk-3-0t64:amd64 3.24.41-4ubuntu1.3	/usr/lib/x86_64-linux-gnu/libgdk-3.so.0.2409.32
lib	link	dpkg:libgtk-3-0t64:amd64 3.24.41-4ubuntu1.3	/usr/lib/x86_64-linux-gnu/libgtk-3.so.0.2409.32
lib	link	dpkg:libjpeg-turbo8:amd64 2.1.5-2ubuntu2	/usr/lib/x86_64-linux-gnu/libjpeg.so.8.2.2
lib	link	dpkg:libsoup-3.0-0:amd64 3.4.4-5ubuntu0.11	/usr/lib/x86_64-linux-gnu/libsoup-3.0.so.0.7.1
lib	link	dpkg:libwebkit2gtk-4.1-0:amd64 2.52.6-0ubuntu0.24.04.1	/usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so.0.21.10
lib	runtime	dpkg:libheif1:amd64 1.17.6-1ubuntu4.9	/usr/lib/x86_64-linux-gnu/libheif.so.1.17.6
lib	runtime	dpkg:libmagickcore-6.q16-7t64:amd64 8:6.9.12.98+dfsg1-5.2build2	/usr/lib/x86_64-linux-gnu/libMagickCore-6.Q16.so.7.0.0
lib	runtime	dpkg:libtiff6:amd64 4.5.1+git230720-4ubuntu2.5	/usr/lib/x86_64-linux-gnu/libtiff.so.6.0.1
lib	runtime	dpkg:libvips42t64:amd64 8.15.1-1.1build4	/usr/lib/x86_64-linux-gnu/libvips.so.42.17.1
```

The licence expressions of the 311 crates, counted:

```text
    129 MIT OR Apache-2.0
     97 MIT
     25 Apache-2.0 OR MIT
     18 Unicode-3.0
      7 MIT/Apache-2.0
      5 MPL-2.0
      3 Unlicense OR MIT
      2 Unlicense/MIT
      2 MIT OR Zlib OR Apache-2.0
      2 BSD-3-Clause
      2 BSD-2-Clause
      2 Apache-2.0/MIT
      2 Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT
      2 Apache-2.0
      1 Zlib
      1 MIT OR Apache-2.0 OR Zlib
      1 MIT AND BSD-3-Clause
      1 ISC
      1 CC0-1.0 OR MIT-0 OR Apache-2.0
      1 BSD-3-Clause/MIT
      1 BSD-3-Clause AND MIT
      1 Apache-2.0 OR BSL-1.0
      1 Apache-2.0 AND MIT
      1 Apache-2.0 / MIT
      1 0BSD OR MIT OR Apache-2.0
      1 (MIT OR Apache-2.0) AND Unicode-3.0
      1 (Apache-2.0 OR MIT) AND BSD-3-Clause
```

Exit status 2 is the script working as intended: Ubuntu's libvips package carries the ImageMagick loader module (`vips-magick.so`), which [ADR 006](../../../../../../resources/architecture/006-libvips-native-stills-not-imagemagick.md) says the inventory must exclude.

## What the owner needs to decide or know

1. **The disk image contains no libvips.** The media workers load libvips at run time by name (`libvips.42.dylib`, then `/opt/homebrew/lib/libvips.42.dylib`; `LIBRARY_NAMES` in `crates/native/src/workers/media/vips.rs`). The bundling overlay adds no library to the app. So the installed app makes previews only on a Mac that has libvips from Homebrew, and the libvips in the review's inventory is the one on the build Mac, not one inside the DMG. [ADR 011](../../../../../../resources/architecture/011-native-media-adapters-and-ffmpeg-license.md) says the native adapters are "all bundled in per-OS installers (not PATH)". Putting libvips and its libraries inside the app is not in this row's text and cannot be built or tested on Linux. I did not change the ADR or the code. Decision needed: bundle libvips into the DMG in a follow-up row before FS-3.1 closes, or ship the early-access DMG with libvips as a stated prerequisite.
2. **ImageMagick in the inventory.** Homebrew's `vips` formula may bring the ImageMagick loader, as Ubuntu's package does. The workers block the Magick operations at run time, but ADR 006 excludes ImageMagick from the inventory itself. Step 5 will show which it is on the Mac.
3. **`hardenedRuntime` false** (above).
4. **The app icon** is my arrangement of the existing mark on a white rounded square. Replace `icons/app-icon.svg` and rerun `npx tauri icon` if the owner has an official app icon.
5. **The About statement names macOS on every OS.** `settingsBuildNote` is shown unconditionally, so a Linux or Windows build (DE-F3.3) will also say "This macOS build…". That is UI work for the UI lane.
6. **Two `libraries.e2e.mjs` tests fail on Linux before this branch's change** (above).

## Owner's macOS test steps (not executed on this machine)

None of these commands has been run. They are written from the Linux runs above, the pinned CLI's help and the Tauri macOS guide. Please report anything that differs.

Before starting: Xcode Command Line Tools (`xcode-select --install`), `rustup` (the workspace pins its toolchain), Node 22.12 or later, and this branch checked out. For previews, libvips from Homebrew (`brew install vips`).

### 1. Build the disk image

```sh
cd apps/filesquid/ui
npm ci
npm run desktop:build -- --config tauri.filesquid.json --config tauri.bundle.macos-dmg.json
```

Expect: the UI build, a release build of `filesquid-desktop`, then lines naming `FileSquid.app` and a `.dmg` under `apps/filesquid/target/release/bundle/` (`macos/FileSquid.app` and `dmg/FileSquid_0.1.0_aarch64.dmg`, or `_x64` on an Intel Mac). macOS may ask to let the terminal control Finder while the bundler lays out the disk image window; allow it.

Send back: the last 20 lines of the output, and the exact `.dmg` file name.

### 2. Check the signature

```sh
cd apps/filesquid/target/release/bundle/macos
codesign -dv --verbose=4 FileSquid.app
codesign --verify --deep --strict --verbose=2 FileSquid.app
spctl --assess --type execute -vv FileSquid.app
```

Expect from the first command: `Identifier=tech.filesquid.desktop`, `Signature=adhoc`, `TeamIdentifier=not set`, and no `runtime` in the `flags` of the `CodeDirectory` line. From the second: `valid on disk` and `satisfies its Designated Requirement`. From the third: `rejected`, because the app is not notarised; that is the expected early-access state.

Send back: the full output of the three commands.

### 3. Install and look at the app

1. Open the `.dmg` in Finder and drag FileSquid to Applications.
2. Open FileSquid from Applications. A build made on the same Mac opens without a Gatekeeper prompt. To see what a downloader sees, send the `.dmg` to yourself (for example through a browser download), open that copy, and use System Settings → Privacy & Security → Open Anyway when macOS blocks it. The install guide of Task FS-3.1 needs the exact wording macOS shows.
3. Check: the Finder and Dock icon is the FileSquid mark on a white rounded square; the menu bar says FileSquid; the window title is FileSquid; the sidebar shows the logo mark and "FileSquid".
4. Open Settings. Under "About FileSquid" check the logo, "FileSquid 0.1.0" and the line "This macOS build is not signed or notarised by Apple yet."
5. Check that "early access" appears nowhere in the app.
6. Create a library, import a JPEG or PNG and check that its thumbnail appears. A thumbnail means the app loaded libvips. If none appears, note whether Homebrew's libvips is installed.

Send back: a screenshot of the About box and of the Dock icon, what macOS said at step 2, and whether the thumbnail appeared.

### 4. Run the packaged-window tests on macOS

```sh
cd apps/filesquid/ui
npm run desktop:build -- --debug --no-bundle --config tauri.filesquid.json --features test-webdriver
cd ../crates/desktop/tests/e2e
npm ci
npx wdio run ./wdio.conf.mjs --spec specs/branding.e2e.mjs
npm run test:e2e
```

Expect from the branding spec: `4 passing`. The whole suite (`npm run test:e2e`) had two failing tests in `libraries.e2e.mjs` on Linux; say whether they fail on macOS too. The test-instrumented binary is a debug build with a WebDriver server in it. It is not the app inside the DMG, which has none; step 3 is the check of that app.

Also run the configuration and inventory tests, which use `otool` on macOS:

```sh
cd apps/filesquid
cargo test -p filesquid-desktop --locked --test integration_distribution_overlay --test integration_linked_inventory
```

Expect: `15 passed` and `13 passed` (five tests run on Linux only: two build an executable with `rustc` against their own shared library, three put a stand-in `ldd` on `PATH`).

Send back: the output of the three test commands.

### 5. Produce the linked inventory for the ADR 011 review

```sh
cd apps/filesquid
bash crates/desktop/scripts/linked-inventory.sh target/release/bundle/macos/FileSquid.app > "$HOME/Desktop/filesquid-dmg-inventory.txt"
echo "exit status: $?"
brew list --versions vips
vips --vips-config
```

Expect: exit status 0, a `file` row for `Contents/MacOS/filesquid-desktop`, `lib` rows marked `system` for the macOS frameworks and `homebrew:<formula> <version>` for libvips and what it loads, `vips` and `module` rows, and a few hundred `crate` rows. Exit status 2 means ImageMagick is in the inventory; exit status 3 means no libvips was found; exit status 1 with a `finding unresolved`, `uninspected`, `runtime-missing` or `crates-unlisted` row means the inventory is incomplete. Check that the `# main executable:` line names `FileSquid.app/Contents/MacOS/filesquid-desktop`. Each is a finding for the review, not a script failure. `finding unresolved` rows name a library the script could not find; the macOS path of the script has never been run, so please report those.

Send back: `filesquid-dmg-inventory.txt`, the exit status, and the output of the two `brew`/`vips` commands. That file is the input to the ADR 011 licence and patent review, which MUST pass before the DMG goes to anyone outside the development team (ADR 035 Decision 5).

## Coverage

The repository sets no coverage threshold for the desktop crate, and no coverage tool ran. This branch adds no code under `src/`. Each key of the two overlay files is asserted by a test in `distribution_overlay.rs`. In the inventory script, the Linux path, the stand-in libvips and module path, the ImageMagick finding and the three refusals are exercised by `linked_inventory.rs`; the macOS path (`otool`, install-name and run-path resolution along the loading chain with each run path expanded relative to the image that declares it, the dylib's own install name left out, Homebrew owners) is not exercised by anything yet.

## Mocked Environment Variables

None. The packaged-window run used the suite's own defaults (a run directory for the app data and the media library).

## Hook

The head-commit run (`FULL=1 bash scripts/pre-commit.sh`, summary line `Test scope: full`) is recorded in the pull request.
