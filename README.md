# Threshold

A first-person Rust prototype set in an impossible apartment. Both doors in the vestibule lead to one kitchen. The amber door keeps your size. The blue door shrinks you and the ball to a tenth of your original size, so the kitchen appears ten times larger. The green door leads to the gallery.

## Play

On Windows x64, download **Threshold-Windows-x64.zip** from the [releases page](https://github.com/viirtualp1/threshold/releases), extract it, and open **threshold.exe**. The Windows build supports Direct3D 12 and Vulkan.

To build from source on Windows or macOS, run Cargo from this folder. The Windows MSVC toolchain also needs the Visual Studio C++ build tools.

```sh
cargo run --release --locked
```

| Control | Action |
| --- | --- |
| Click the game window | Capture the mouse |
| WASD and Shift | Move and sprint |
| Mouse or arrow keys | Look |
| E or left click | Pick up the ball when the reticle highlights, then throw it |
| Esc | Release the mouse |
| R | Reset the apartment |
| F | Toggle fullscreen |

Try both kitchen doors. For a second experiment, grab the ball off its pedestal, throw it through the blue door, then follow it in. You can return through the same doorway behind you.

## How it works

- `scene.rs` owns the room-local layout and paired doorway transforms. Each room has one set of furniture and one set of bounds.
- `sim.rs` advances the player and ball at 240 Hz. Threshold crossings transform positions, headings, velocities, and physical size. A ball that overlaps a doorway is visible from either side.
- `room.wgsl` traces camera rays through up to eight doorway transitions before shading the hit. The renderer keeps one static scene buffer and draws into a pixel-budgeted target. A second pass draws the HUD at window resolution.
- `cargo test --locked` checks portal round trips, both kitchen entries, the gallery, pickup and throw, and wall and floor collisions.

The prototype uses one interactive ball with fixed furniture and room-local lighting.
