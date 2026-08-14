# nat3d-tui

**Suspended, not abandoned.** This crate is excluded from the workspace build
and is not part of any release. It is kept in the tree because the work in it is
sound and finishing it is a small, well-defined job; it is kept out of the build
because shipping it as it stands would misrepresent what NAT3D does.

## Why it is suspended

The menu offers eight actions:

| Action | Subcommand it invokes | Exists in `nat3d-cli` |
|---|---|---|
| Render | `render` | yes |
| Generate | `generate` | no |
| Convert | `convert` | no |
| Batch | `batch` | no |
| Info | `info` | no |
| Validate | `validate` | no |
| Script | `script` | no |
| Benchmark | `benchmark` | no |

`nat3d-cli` implements exactly one subcommand, `render`. A user opening this
interface would be shown eight capabilities and would find that seven of them do
nothing. That is the same kind of claim-without-implementation that removed two
render engines from this repository, and it is not shipped for the same reason.

There is a second, independent defect: the binary is resolved as `nat3d.exe`,
a Windows name, while the executable produced by `nat3d-cli` is called `nat3d`.
Every action therefore fails on Linux, macOS and Android even where the
subcommand does exist.

## What finishing it requires

Implementing the seven missing subcommands in `nat3d-cli`, and resolving the
binary by platform rather than by a hardcoded Windows filename. Alternatively,
reducing the menu to the actions that exist, which is a much smaller change and
would make the crate shippable immediately at the cost of being a one-item menu.

Neither is a cleanup task, so neither was done as part of one.

## Building it anyway

```bash
cargo build --release --manifest-path crates/nat3d-tui/Cargo.toml
```

It compiles without warnings and passes `clippy -D warnings`. The interface
itself, the navigation and the rendering are functional; what is missing is
behind the menu, not in it.
