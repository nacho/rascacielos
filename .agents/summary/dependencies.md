# Dependencies

## Crates (`Cargo.toml`)

| Crate | Features | Used for |
|---|---|---|
| `gtk4` (as `gtk`) | `v4_22` | Widgets, snapshot drawing, printing |
| `libadwaita` (as `adw`) | `v1_9` | Application, dialogs, breakpoints, toggle groups, style manager |
| `glib` | `log_macros` | `glib::debug!` and `glib::warn!` with the `rascacielos` domain |
| `gettext-rs` | `gettext-system` | Translations |
| `serde`, `serde_json` | `derive` | JSON saves and the data file |
| `fastrand` | none | Random numbers for the generator |
| `glib-build-tools` (build) | none | Compiling the GResource in `build.rs` |

The project aims to keep dependencies minimal and well-justified. Each entry
in `Cargo.toml` has a comment saying why it is there.

## System and toolkit

| Requirement | Where it is declared |
|---|---|
| GTK ≥ 4.22, libadwaita ≥ 1.9 | Cargo features `v4_22` and `v1_9`, and the Meson dependency checks |
| GIO ≥ 2.80 | `meson.build` |
| Meson ≥ 1.4, Cargo, gettext | `meson.build` |
| `desktop-file-validate`, `appstreamcli` | Optional Meson tests in `data/meson.build` |
| GNOME 51 runtime + `rust-stable` extension | `build-aux/io.github.nacho.rascacielos.Devel.json` |

Ubuntu 26.04 (GNOME 50) has new enough GTK and libadwaita, and CI uses it.
On older distributions, build inside the GNOME Flatpak SDK.

## Tooling

- `rustfmt` and `clippy` with `-D warnings`, both in CI and in the pre-commit
  hook (`build-aux/pre-commit`, which only checks formatting).
- CI installs Rust with `actions-rs/toolchain@v1`, the same as Teclas and
  Mundi. Keep the workflows of the three apps alike.
