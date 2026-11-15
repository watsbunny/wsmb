# WSMB — Watsbunny Smart Motion Blur

An open-source OFX plugin that adds physically plausible motion blur to footage
by estimating dense motion between neighbouring frames and integrating the image
along the resulting trajectories.

Written in Rust. Licensed under the **GNU GPL v3 or later**.

> Status: early. The core estimator and the blur synthesis run today through a
> dev CLI; the OFX plugin wrapper and the GPU backends are in progress.

## Why

Most motion blur in the wild is either a simple directional blur (a linear box
filter along a single vector) or a post effect that needs motion vectors from
the renderer. WSMB estimates per-pixel motion from the footage itself, so it
works on live action, stock footage, AI-generated video and 3D renders that were
exported without vector passes.

## How it works

```
frames ──► motion estimation ──► flow fields ──► trajectory integration ──► out
```

1. **Motion estimation** (`wsmb-core::flow`) — a coarse-to-fine block matcher.
   Both frames are reduced to a luminance pyramid; the coarsest level is searched
   exhaustively inside a bounded window, every finer level only refines a ±1
   window around the upsampled prediction. A 3×3 median filter per level removes
   impulse outliers. Forward and backward fields are estimated independently.

2. **Shutter integration** (`wsmb-core::blur`) — for each output pixel we solve
   the trajectory position by fixed-point iteration,

   ```
   x(t) = x₀ + t·F(x₀)     ⟹     x₀ = x − t·F(x₀)
   ```

   and resample the two source frames at `N` positions inside the shutter
   interval. Samples are weighted by

   * **forward/backward consistency** — `‖F(x) + G(x + F(x))‖` small means the
     match is trustworthy; large means we are looking at an occlusion or a
     mismatched region, so the sample is down-weighted;
   * a **bilateral colour term** against the mid-shutter colour, which stops the
     blur from bleeding across depth and colour discontinuities.

   The formulation is the classic gather-based flow-guided integration
   `B(x) = ∫ w(t)·I(t, x + t·F(x)) dt / ∫ w(t) dt`.

Everything above is implemented from published technique (block matching,
pyramidal prediction, median regularisation, forward-backward consistency,
bilateral weighting). No code, assets or reverse-engineered internals of any
commercial plugin are used.

## Status

Working today:

- dense forward/backward motion estimation with a pyramidal block matcher
- flow-guided shutter integration with occlusion and colour weighting
- GPU shutter integration through a WGSL compute shader (Vulkan/DX12 via
  wgpu): 1920×1080 at 24 samples went from 3.2 s to 0.41 s on an RX 6750 XT,
  matching the CPU result to 1e-7
- optional `Matte` input: the matte picks the motion multiplier per pixel, so
  foreground and background can blur by different amounts
- optional `Vectors` input: red/green carry the displacement in pixels and
  replace the estimate entirely
- an OFX 1.4 plugin that EffectCraft 0.24.1 loads, describes and renders
  (`CreateInstance → GetClipPreferences → GetRoD → GetRoI → IsIdentity →
  BeginSequenceRender → Render → EndSequenceRender`)
- a dev CLI that renders blur between two BMPs, used for headless checks

Known limits:

- **Search range.** The matcher can see at most `search_radius · 2^(levels-1)`
  pixels of motion: the default 5 levels × 8 search covers ~128 px. A wider
  radius is nearly free because it only widens the coarsest level.
- **Flat regions.** A block matcher has nothing to lock onto inside a large
  uniform area, so its interior keeps the predicted (near zero) motion and
  barely blurs — only the edges smear. Fixing this needs a smoothness term
  (flow diffusion or variational refinement), not a bigger block: a 5×5 or 7×7
  window makes it worse on small frames.
- **Temporal access.** EffectCraft's OFX helper serves time-invariant clip
  images in headless renders, so blur is not visible there for any plugin
  (RE:Vision's RSMB behaves the same way in the same test). The plugin handles
  it correctly by falling back to "no motion, no blur"; verifying visually
  needs the GUI or a host that honours sub-frame clip access.

## Repository layout

```
crates/
  wsmb-core/   pure Rust, no dependencies: frames, flow, blur, params
  wsmb-gpu/    WGSL compute shader for the shutter integration (wgpu)
  wsmb-cli/    dev CLI, renders blur between two BMPs
  wsmb-ofx/    the OFX plugin (cdylib)
```

## Build

Requires the Rust toolchain and, on Windows, the MSVC linker (Visual Studio
Build Tools with the *Desktop development with C++* workload).

```powershell
rustup default stable
cargo test
cargo build --release
```

## Try it from the CLI

```powershell
cargo run --release -p wsmb-cli -- prev.bmp next.bmp out.bmp --shutter 0.5 --samples 24
```

| flag | meaning | default |
| --- | --- | --- |
| `--shutter` | shutter duration as a fraction of the frame interval | `0.5` |
| `--samples` | integration samples inside the shutter | `16` |
| `--amount` | global motion multiplier | `1.0` |
| `--t` | render time between the two frames | `0.5` |
| `--mix` | blend with the unblurred frame | `1.0` |
| `--occlusion` | weight of the consistency term | `0.8` |
| `--sigma` | bilateral colour sigma, `0` disables | `0.15` |
| `--gpu` | run the integration on the GPU | off |
| `--levels` | pyramid depth | `5` |
| `--search` | search radius on the coarsest level | `8` |
| `--block` | block radius | `1` |

Plugin parameters: `shutter`, `samples`, `amount`, `fgamount`, `mix`,
`occlusion`, `quality`, `usegpu`. Optional inputs: `Matte`, `Vectors`.

## Roadmap

- [x] Dense forward/backward motion estimation
- [x] Flow-guided shutter integration with occlusion handling
- [x] Dev CLI for headless testing
- [x] OFX plugin wrapper (`ofxCore`, `ofxImageEffect`, `ofxParam`, `ofxProperty`)
- [x] GPU backend (Vulkan/DX12 via `wgpu`) for the shutter integration
- [x] External motion-vector input, for renders that ship vector passes
- [x] Foreground/background split driven by a matte
- [ ] GPU motion estimation (flow is ~60% of the frame time now)
- [ ] Flow diffusion through flat regions
- [ ] Multi-frame integration (fetch more than two neighbours from the host)
- [ ] Half/float pixel formats, tiles and RoD support
- [ ] Regression suite comparing CPU vs GPU output

## Host support

The target host is **EffectCraft**; any OFX 1.4 host should work once the wrapper
lands. On Windows the bundle goes to
`C:\Program Files\Common Files\OFX\Plugins\WSMB.ofx.bundle`.

## Notes on the OFX side

Three things cost real time while wiring the plugin up, worth writing down:

1. **`describe` gets a descriptor handle, not a property set.** Calling the
   property suite on it directly returns `kOfxStatErrBadHandle` (9) for every
   setter. You have to call `OfxImageEffectSuiteV1::getPropertySet` first.
2. **Multi-value properties are set per index.** `propSetString(key, 0, …)`,
   `propSetString(key, 1, …)`; the `*N` variants work in some hosts and not in
   others.
3. **`clipGetImage` at other times needs `kOfxImageEffectPropTemporalClipAccess`
   on the clip descriptor**, and the host has to actually serve those frames.

`WSMB_LOG=1` in the host's environment makes the plugin append every action it
receives to `%TEMP%\wsmb-ofx.log`.

## Development

```powershell
cargo test                                   # core + plugin unit tests
cargo clippy --workspace --all-targets
.\scripts\build-plugin.ps1                   # builds dist\WSMB.ofx.bundle
.\scripts\link-bundle.ps1                    # (elevated) links the bundle into Program Files
```

EffectCraft renders headlessly, which is how the plugin was exercised:

```powershell
$cli = "C:\Program Files\ArtCraft\EffectCraft-OFX\effectcraft-cli.exe"
& $cli --demo run --commands .\scratch\render-blur.json --json
```

## Licence

GPL-3.0-or-later. See [LICENSE](LICENSE).

WSMB is an independent project. It is not affiliated with, endorsed by, or
derived from any commercial motion blur product. Product names mentioned in
documentation belong to their respective owners.
