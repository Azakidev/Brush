mime_types = ['application/x-brush']

conf = configuration_data()
conf.set('APP_ID', app_id)
conf.set('BIN_NAME', bin_name)
conf.set('SUFFIX', get_option('app_id_suffix'))
conf.set('MIME_TYPES', ';'.join(mime_types) + ';')

desktop_file = i18n.merge_file(
  input: configure_file(
    input: 'art.FatDawlf.Brush.desktop.in.in',
    output: '@0@.desktop.in'.format(app_id),
    configuration: conf,
  ),
  output: '@0@.desktop'.format(app_id),
  type: 'desktop',
  po_dir: '../po',
  install: true,
  install_dir: get_option('datadir') / 'applications',
)

desktop_utils = find_program('desktop-file-validate', required: false)
if desktop_utils.found()
  test('Validate desktop file', desktop_utils, args: [desktop_file])
endif

appstream_file = i18n.merge_file(
  input: 'art.FatDawlf.Brush.metainfo.xml.in',
  output: 'art.FatDawlf.Brush.metainfo.xml',
  po_dir: '../po',
  install: true,
  install_dir: get_option('datadir') / 'metainfo',
)

appstreamcli = find_program('appstreamcli', required: false, disabler: true)
test(
  'Validate appstream file',
  appstreamcli,
  args: ['validate', '--no-net', '--explain', appstream_file],
)

install_data(
  'art.FatDawlf.Brush.gschema.xml',
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas',
)

compile_schemas = find_program('glib-compile-schemas', required: false, disabler: true)
test(
  'Validate schema file',
  compile_schemas,
  args: ['--strict', '--dry-run', meson.current_source_dir()],
)

service_conf = configuration_data()
service_conf.set('bindir', get_option('prefix') / get_option('bindir'))
configure_file(
  input: configure_file(
    input: 'art.FatDawlf.Brush.service.in.in',
    output: '@0@.service.in'.format(app_id),
    configuration: conf,
  ),
  output: '@0@.service'.format(app_id),
  configuration: service_conf,
  install_dir: get_option('datadir') / 'dbus-1' / 'services',
)

# MIME type definition
install_data(
  'art.FatDawlf.Brush.xml',
  install_dir: get_option('datadir') / 'mime' / 'packages',
)

# Thumbnailer entry
install_data('brush.thumbnailer', install_dir: get_option('datadir') / 'thumbnailers')

subdir('icons')
