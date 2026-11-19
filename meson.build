project(
  'rascacielos',
  'rust',
  license: 'GPL-3.0-or-later',
  meson_version: '>= 1.4',
  version: '0.1.0'
)

gnome = import('gnome')
i18n = import('i18n')

datadir     = get_option('datadir')
localedir   = get_option('localedir')
prefix      = get_option('prefix')
bindir      = get_option('bindir')
profile     = get_option('profile')

base_id = 'io.github.nacho.rascacielos'
app_id = base_id
if profile == 'Devel'
  app_id += '.' + profile
endif

dependency('gio-2.0', version: '>= 2.80.0')
dependency('gtk4', version: '>= 4.22')
dependency('libadwaita-1', version: '>= 1.9')
cargo = find_program('cargo')

gettext_package = meson.project_name()

cargo_options = ['--manifest-path', meson.project_source_root() / 'Cargo.toml']
cargo_options += ['--target-dir', meson.project_build_root() / 'target']
if profile == 'Devel'
  rust_target = 'debug'
else
  cargo_options += ['--release']
  rust_target = 'release'
endif

cargo_env = environment()
cargo_env.set('APP_ID', app_id)
cargo_env.set('VERSION', meson.project_version())
cargo_env.set('GETTEXT_PACKAGE', gettext_package)
cargo_env.set('LOCALEDIR', prefix / localedir)

cargo_build = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  env: cargo_env,
  command: [
    'sh', '-c',
    '"$0" build "$@" && cp "@0@" "@OUTPUT@"'.format(
      meson.project_build_root() / 'target' / rust_target / meson.project_name()),
    cargo, cargo_options,
  ],
)

test(
  'cargo-test',
  cargo,
  args: ['test', cargo_options],
  env: cargo_env,
  timeout: 600,
)

subdir('data')
subdir('po')

gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)
