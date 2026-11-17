To build Rascacielos you need `meson`, `blueprint-compiler`, `cargo` and
`rust`, and the development libraries for GTK 4 and libadwaita 1.10 or newer.

The simplest way is to use the GNOME Flatpak SDK through GNOME Builder or
`flatpak-builder`:

```
flatpak-builder --user --install --force-clean _build-flatpak build-aux/io.github.nacho.rascacielos.Devel.json
flatpak run io.github.nacho.rascacielos.Devel
```

With the libraries installed on the host:

```
meson setup builddir -Dprofile=Devel
meson compile -C builddir
GSETTINGS_SCHEMA_DIR=builddir/data ./builddir/src/rascacielos
```
