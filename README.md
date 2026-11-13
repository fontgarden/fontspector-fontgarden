# fontspector-fontgarden

A [Fontspector](https://github.com/fonttools/fontspector) plugin with the
Font.Garden profile. The profile `fontgarden` is the Google Fonts profile with
the Font.Garden requirements added.

| Check | Requirement |
| --- | --- |
| `fontgarden/unitsperem` | The em is 1024 units. |
| `fontgarden/grid` | Every master of the variable font has its on-curve points and advance widths on the 2-unit grid. |
| `fontgarden/license` | Name ID 13 gives the SIL Open Font License 1.1, and name ID 14 is `https://openfontlicense.org`. |
| `opentype/vendor_id` | The OS/2 vendor ID is `FTGD`. This existing check is configured by the profile. |

The grid and the 1024-unit em are described in
[Virtua Grotesk](https://elih.net/blog/virtua-grotesk/).

## Use

Download the binary for your platform from the
[releases](https://github.com/fontgarden/fontspector-fontgarden/releases), or
build it:

```sh
cargo build --release
```

Then run Fontspector with the plugin and the profile:

```sh
fontspector --plugin target/release/fontgarden-fontspectorplugin -p fontgarden FONT...
```

A release of this plugin is built for one Fontspector version, given in
`Cargo.toml`. Use that Fontspector version.

## Limits of a check of compiled fonts

- The conversion to quadratic curves moves the off-curve points, so the grid
  check reads only on-curve points. Check the sources for the off-curve points.
- Static fonts are skipped by the grid check. They are instances, mostly
  between masters.
- The grid check rounds coordinates first. An optimized gvar table can put a
  master up to half a unit off its source.

## Add a requirement

If an existing Fontspector check can test the requirement, configure that
check in `src/lib.rs`. Otherwise add a check to `src/checks.rs` and register
it in `src/lib.rs`. Put the reason in the rationale of the check.

## License

Apache License 2.0. See [LICENSE](LICENSE).
