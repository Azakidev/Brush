application_id = 'art.FatDawlf.Brush'

scalable_dir = 'hicolor' / 'scalable' / 'apps'
install_data(
  scalable_dir / ('@0@.svg').format(application_id),
  install_dir: get_option('datadir') / 'icons' / scalable_dir
)

symbolic_dir = 'hicolor' / 'symbolic' / 'apps'
install_data(
  symbolic_dir / ('@0@-symbolic.svg').format(application_id),
  install_dir: get_option('datadir') / 'icons' / symbolic_dir
)

actions_dir = 'hicolor' / 'scalable' / 'actions'
install_data(
  actions_dir / 'paintbrush-symbolic.svg',
  actions_dir / 'box-dotted-symbolic.svg',
  actions_dir / 'move-tool-symbolic.svg',
  actions_dir / 'pointer-secondary-click-symbolic.svg',
  actions_dir / 'box-outline-symbolic.svg',
  actions_dir / 'circle-anchor-center-symbolic.svg',
  actions_dir / 'circle-dotted-symbolic.svg',
  actions_dir / 'magic-wand-symbolic.svg',
  actions_dir / 'double-ended-arrows-horizontal-symbolic.svg',
  install_dir: get_option('datadir') / 'icons' / actions_dir
)
