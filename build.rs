/* -*- Mode: rust; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/*
 * Copyright © 2026 Ignacio Casal Quinteiro
 *
 * This file is part of Rascacielos.
 *
 * Rascacielos is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Rascacielos is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Rascacielos. If not, see <http://www.gnu.org/licenses/>.
 */

use glib_build_tools::compile_resources;
use std::env;
use std::fs;
use std::path::Path;

/// Variables substituted in the `.in` templates. Meson passes the real
/// values through the environment; plain Cargo builds use these defaults.
const VARIABLES: &[(&str, &str)] = &[
    ("APP_ID", "io.github.nacho.rascacielos.Devel"),
    ("VERSION", env!("CARGO_PKG_VERSION")),
    ("GETTEXT_PACKAGE", "rascacielos"),
    ("LOCALEDIR", "/usr/share/locale"),
];

fn substitute(template: &str, marker: impl Fn(&str) -> String) -> String {
    let mut content = template.to_string();
    for (name, default) in VARIABLES {
        let value = env::var(name).unwrap_or_else(|_| default.to_string());
        content = content.replace(&marker(name), &value);
    }
    content
}

fn generate(template_path: &str, dest: &Path, marker: impl Fn(&str) -> String) {
    let template = fs::read_to_string(template_path)
        .unwrap_or_else(|e| panic!("Failed to read {template_path}: {e}"));
    fs::write(dest, substitute(&template, marker)).unwrap();
    println!("cargo:rerun-if-changed={template_path}");
}

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);

    for (name, _) in VARIABLES {
        println!("cargo:rerun-if-env-changed={name}");
    }

    generate("src/config.rs.in", &out_dir.join("config.rs"), |name| {
        format!("@{name}@")
    });

    // The about dialog reads the metainfo from the resources. Meson also
    // installs a translated copy of it.
    generate(
        "data/io.github.nacho.rascacielos.metainfo.xml.in.in",
        &out_dir.join("metainfo.xml"),
        |name| match name {
            "APP_ID" => "@app-id@".to_string(),
            _ => format!("@{name}@"),
        },
    );

    compile_resources(
        &[Path::new("resources"), out_dir],
        "resources/resources.gresource.xml",
        "resources.gresource",
    );
}
