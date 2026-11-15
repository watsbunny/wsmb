# What the RSMB 7 binary exposes

Notes from a structural look at `rsmb.ofx` (ReelSmart Motion Blur 7.0.0, Win64,
9.6 MB, stripped). The goal was to understand **how the pipeline is organised**,
not to lift its implementation: the compiled OpenCL kernels are not present as
readable source (see below), and nothing in this repository is derived from
their code.

## Tooling

- Ghidra 12.2 headless, default analyzers, ~83 s.
- 6 823 functions, image base `180000000`.
- Plus a plain string dump of the binary (11 679 strings).

## Imports tell the story

Graphics and compute are wired together through OpenCL/OpenGL sharing:

```
clCreateFromGLTexture2D, clEnqueueAcquireGLObjects, clEnqueueReleaseGLObjects
clCreateProgramWithSource, clBuildProgram, clCreateKernel, clSetKernelArg
clEnqueueNDRangeKernel, clEnqueueReadImage, clEnqueueWriteImage
glGenTextures, glBindTexture, glTexImage2D, glDrawElements, glFinish
CreateThread, InitializeCriticalSectionEx, AcquireSRWLockExclusive
```

So: GPU work runs as OpenCL kernels over OpenGL textures, with a CPU thread pool
alongside, and a GL fallback drawing path.

## Kernels are built at runtime

There are **zero** occurrences of `__kernel` or `kernel void` in the file, while
`clCreateProgramWithSource` is called. The OpenCL source is generated or
reconstructed at runtime, so there is nothing to read statically. The kernel
*names* are plain strings, though, and they describe the pipeline:

| Group | Names | Reading |
| --- | --- | --- |
| Gradients | `OCLImageCalcGrad`, `OCLImageCalcGradAndA360`, `OCLImageCalcB`, `OCLImageCalcBUserC360` | per-frame gradient / right-hand-side terms |
| Estimation | `oclImage_AdjustVectorsWithBlockMatch1..4` | **four** block-matching passes |
| Coverage | `oclImage_AdjustVectorsWCov1..4`, `OCLImageClampCoverage`, `OCLImageClearCoverageToFull` | per-pass confidence map and its fill |
| Regularisation | `gaussianBlurH/V5CLImage`, `gaussianBlurH/V7CLImage`, `...2` | separable 5- and 7-tap Gaussian on the field |
| Pyramid | `subSampleFilterH/VCLImage` | downsample filters |
| Borders | `OCL_AdjustVectors_CopyTopBottomScanlines`, `CopyLeftRightColumns` | wrap handling |
| Integration | `motionBlurHigh/Low`, `motionBlurHigh/LowZero`, `...Crop`, `...360` | final accumulate, high/low sample counts |
| Validation | `OCLImageInvWarp_NoCov_Bilinear360` | inverse warp for checking |

`reSampleAndScale360Image` and the `_360` suffix everywhere show that equirect
footage gets its own sampling path.

## OFX contract they rely on

```
OfxImageEffectActionDescribeInContext, ...IsIdentity, ...Render,
...GetRegionOfDefinition, ...GetRegionsOfInterest, ...GetClipPreferences,
...GetOutputColourspace, ...GetTimeDomain, ...BeginSequenceRender,
...EndSequenceRender, ...GetFramesNeeded
OfxImageEffectPropTemporalClipAccess
```

`GetFramesNeeded` is the interesting one: it is how a plugin asks the host for
neighbouring frames. EffectCraft's OFX helper does not implement that action
(the string is absent from `effectcraft-helper-ofx.exe`), which is why neither
RSMB nor WSMB sees motion in its headless renders.

## What we changed in WSMB because of this

The structure — *match, smooth the field, match again, keep a coverage map* —
is exactly what our estimator was missing. It is also standard coarse-to-fine
practice (pyramidal block matching, Horn–Schunck-style regularisation,
Lucas–Kanade refinement), so we implemented it from the literature:

- `dense_flow` now runs `refine_passes` block matches per level instead of one.
- Between them the field goes through a 5-tap separable Gaussian
  (`FlowField::smooth`), which carries confident motion into flat areas.

Effect on the synthetic 60 px shift test: a solid rectangle that previously
came out almost unblurred now smears across the full trajectory, because the
neighbouring edge motion diffuses into its interior.

Cost at 1920×1080: flow 660 ms → 1.5 s on CPU (two passes), integration
unchanged at ~0.3 s on the GPU.

## What we deliberately did not do

- No extraction of the runtime-generated kernel source.
- No transcription of decompiled code into this repository.
- No use of their assets, identifiers or parameter ids.

Everything in `wsmb-*` is written in Rust from published technique; the binary
served as a map of *what* a production motion blur does, not as a source of
*how* to write it.
